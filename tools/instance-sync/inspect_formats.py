"""Read-only format evidence. Never opens launcher data or starts a JVM/game.

Only caller-supplied Client source and Gradle artifact caches are read. Outputs
are evidence, not a runtime allowlist. Unknown codecs must remain local.
"""
import argparse
import hashlib
import json
import pathlib
import re
import struct
import subprocess
import urllib.request
import zipfile


class ClassFile:
    """Small JVM class reader; resolves constants, fields and instruction operands."""

    def __init__(self, data):
        self.data, self.pos = data, 0
        assert self.take(4) == b"\xca\xfe\xba\xbe"
        self.take(4)
        self.pool = [None] * self.uint(2)
        i = 1
        while i < len(self.pool):
            tag = self.uint(1)
            if tag == 1:
                value = self.take(self.uint(2)).decode("utf-8", errors="replace")
            elif tag in (3, 4):
                value = struct.unpack(">i" if tag == 3 else ">f", self.take(4))[0]
            elif tag in (5, 6):
                value = struct.unpack(">q" if tag == 5 else ">d", self.take(8))[0]
            elif tag in (7, 8, 16, 19, 20):
                value = self.uint(2)
            elif tag in (9, 10, 11, 12, 17, 18):
                value = (self.uint(2), self.uint(2))
            elif tag == 15:
                value = (self.uint(1), self.uint(2))
            else:
                raise ValueError(f"unsupported constant {tag}")
            self.pool[i] = (tag, value)
            i += 2 if tag in (5, 6) else 1
        self.take(6)
        self.take(2 * self.uint(2))
        self.fields = self.members()
        self.methods = self.members()

    def take(self, size):
        result = self.data[self.pos:self.pos + size]
        assert len(result) == size
        self.pos += size
        return result

    def uint(self, size):
        return int.from_bytes(self.take(size), "big")

    def resolve(self, index):
        tag, value = self.pool[index]
        if tag in (7, 8, 16):
            return self.resolve(value)
        if tag in (9, 10, 11):
            return [self.resolve(v) for v in value]
        if tag == 12:
            return [self.resolve(v) for v in value]
        if tag == 18:
            return ["bootstrap", value[0], self.resolve(value[1])]
        if tag == 15:
            return ["handle", value[0], self.resolve(value[1])]
        return value

    def members(self):
        members = {}
        for _ in range(self.uint(2)):
            flags, name, desc = self.uint(2), self.resolve(self.uint(2)), self.resolve(self.uint(2))
            attrs = {}
            for _ in range(self.uint(2)):
                key = self.resolve(self.uint(2))
                attrs[key] = self.take(self.uint(4))
            members[(name, desc)] = (flags, attrs)
        return members

    def instructions(self, name):
        for (method, _), (_, attrs) in self.methods.items():
            if method != name or "Code" not in attrs:
                continue
            raw = attrs["Code"]
            code = raw[8:8 + int.from_bytes(raw[4:8], "big")]
            p, result = 0, []
            while p < len(code):
                start, op = p, code[p]
                p += 1
                if op in (170, 171):
                    p += (-p) % 4
                    p += 4
                    if op == 170:
                        low, high = struct.unpack(">ii", code[p:p + 8])
                        p += 8 + 4 * (high - low + 1)
                    else:
                        count = int.from_bytes(code[p:p + 4], "big")
                        p += 4 + count * 8
                elif op == 196:
                    p += 5 if code[p] == 132 else 3
                elif op in (185, 186, 200, 201):
                    p += 4
                elif op == 197:
                    p += 3
                elif op in (17, 19, 20, 132) or 153 <= op <= 168 or 178 <= op <= 184 or op in (187, 189, 192, 193, 198, 199):
                    p += 2
                elif op in (16, 18, 169, 188) or 21 <= op <= 25 or 54 <= op <= 58:
                    p += 1
                operand = code[start + 1:p]
                if op in (18, 19, 20) or 178 <= op <= 186 or op in (187, 189, 192, 193, 197):
                    index = int.from_bytes(operand[:1 if op == 18 else 2], "big")
                    value = self.resolve(index)
                else:
                    value = operand.hex()
                result.append((start, op, value))
            return result
        return []

    def options(self):
        result = {}
        for method in ("processOptions", "processDumpedOptions"):
            instructions = self.instructions(method)
            for i, (offset, op, value) in enumerate(instructions):
                if op not in (18, 19) or not isinstance(value, str):
                    continue
                # Literal field keys have a direct receiver-field load, then
                # process(). Dynamic key/model/sound loops are handled apart.
                following = instructions[i + 1:i + 10]
                field = next((v for _, o, v in following if o == 180), None)
                call = next((v for _, o, v in following if o == 185), None)
                if field and call and call[1][0] == "process":
                    result[value] = {"field": field[1][0], "descriptor": field[1][1],
                                     "access": call[1][1], "method": method, "offset": offset}
        return result


def sha(data, algorithm="sha256"):
    return hashlib.new(algorithm, data).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--client", type=pathlib.Path, required=True)
    parser.add_argument("--loom", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--verify-official", action="store_true", help="verify cached metadata against official digest-bound HTTPS documents")
    args = parser.parse_args()
    evidence = {"minecraft": {}, "aurora": {}}
    manifest = json.loads((args.loom / "mojang_versions_manifest.json").read_bytes())
    manifest = {item["id"]: item for item in manifest["versions"]}
    for version in ("26.1.2", "26.2", "26.3"):
        metadata_bytes = (args.loom / version / "mojang_minecraft_info.json").read_bytes()
        metadata = json.loads(metadata_bytes)
        # Gradle reformats metadata; compare semantics after verifying the
        # original official document against its manifest-provided digest.
        verified = False
        if args.verify_official:
            with urllib.request.urlopen(manifest[version]["url"], timeout=30) as response:
                official = response.read(2 * 1024 * 1024 + 1)
            assert len(official) <= 2 * 1024 * 1024
            assert sha(official, "sha1") == manifest[version]["sha1"]
            assert json.loads(official) == metadata
            verified = True
        jar_bytes = (args.loom / version / "minecraft-client.jar").read_bytes()
        assert sha(jar_bytes, "sha1") == metadata["downloads"]["client"]["sha1"]
        jar = zipfile.ZipFile(args.loom / version / "minecraft-client.jar")
        clazz = ClassFile(jar.read("net/minecraft/client/Options.class"))
        fields = clazz.options()
        initializers, current = {}, []
        for _, op, value in clazz.instructions("<init>"):
            current.append([op, value])
            if op == 181 and value[0] == "net/minecraft/client/Options":
                initializers[value[1][0]] = current
                current = []
        for key, item in fields.items():
            init = initializers.get(item["field"], [])
            item["initializerSha256"] = sha(json.dumps(init, sort_keys=True).encode())
            item["initializer"] = init
            item["fieldSignature"] = next(
                (clazz.resolve(int.from_bytes(a["Signature"], "big"))
                 for (n, _), (_, a) in clazz.fields.items() if n == item["field"] and "Signature" in a), None)
        support = {}
        for name in ("net/minecraft/client/OptionInstance", "net/minecraft/client/OptionInstance$IntRange",
                     "net/minecraft/client/OptionInstance$UnitDouble", "net/minecraft/client/OptionInstance$ClampingLazyMaxIntRange",
                     "com/mojang/blaze3d/platform/InputConstants", "com/mojang/blaze3d/platform/InputConstants$Type",
                     "net/minecraft/client/KeyMapping", "net/minecraft/sounds/SoundSource",
                     "net/minecraft/world/entity/player/PlayerModelPart"):
            data = jar.read(name + ".class")
            support[name] = {"sha256": sha(data), "class": ClassFile(data)}
        enum_owners = {v[0] for x in fields.values() for op, v in x["initializer"]
                       if op == 178 and isinstance(v, list) and v[1][0] in ("CODEC", "LEGACY_CODEC")}
        for owner in enum_owners:
            data = jar.read(owner + ".class")
            support[owner] = {"sha256": sha(data), "class": ClassFile(data)}
        for name, entry in support.items():
            c = entry.pop("class")
            entry["methods"] = {n: c.instructions(n) for n in sorted({n for n, _ in c.methods})
                                if n in ("<clinit>", "codec", "getKey", "getName", "saveString", "getOrCreate", "createBoolean")}
        evidence["minecraft"][version] = {
            "officialMetadata": manifest[version], "officialMetadataVerified": verified,
            "clientSha1": sha(jar_bytes, "sha1"),
            "optionsClassSha256": sha(jar.read("net/minecraft/client/Options.class")),
            "version": json.loads(jar.read("version.json")), "keys": fields, "support": support,
            "vanillaActions": sorted({v for _, op, v in clazz.instructions("<init>")
                                      if op in (18, 19) and isinstance(v, str) and v.startswith("key.")}),
        }
    config_path = "core/src/main/java/com/aurora/client/config/AuroraConfig.java"
    for ref in ("v3.5.0", "HEAD"):
        source = subprocess.check_output(["git", "-C", str(args.client), "show", f"{ref}:{config_path}"]).decode()
        # Only top-level public, mutable data declarations, not enum members.
        fields = []
        for number, line in enumerate(source.splitlines(), 1):
            match = re.match(r"    public (?!static\b|final\b|class\b|enum\b)([\w.<>?, ]+(?:\[\])?)\s+(\w+)\s*=", line)
            if match:
                fields.append({"field": match[2], "javaType": match[1], "line": number})
        evidence["aurora"][ref] = {"commit": subprocess.check_output(["git", "-C", str(args.client), "rev-parse", ref]).decode().strip(),
                                    "configSha256": sha(source.encode()), "fields": fields}
    args.output.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({v: len(d["keys"]) for v, d in evidence["minecraft"].items()}))
    print(json.dumps({v: len(d["fields"]) for v, d in evidence["aurora"].items()}))


if __name__ == "__main__":
    main()
