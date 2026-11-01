"""PANDORA-4 owned-process sampling. Windows logical process I/O, not physical disk.
Run optimized Engine probes sequentially; no public network or installed games.
The native mode crosses one real minute tick in a marked copy of the same fixture.
"""
import argparse, hashlib, json, os, shutil, statistics, subprocess, time
from pathlib import Path
from pandora_resources import setup, metrics


def delta(a,b):
    return {key:b[key]-a[key] for key in ["cpuSeconds","logicalReadBytes","logicalWriteBytes"]}


def engine(exe,root,n):
    kernel,psapi=setup()
    with (root.parent/(root.name+"-stderr.txt")).open("wb") as err:
        p=subprocess.Popen([str(exe),str(root),str(n)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True)
        try:
            cold=json.loads(p.stdout.readline());ready=json.loads(p.stdout.readline())
            if not ready.get("ready"):raise RuntimeError("fixture did not initialize")
            samples=[];idle_start=time.perf_counter()
            for _ in range(40):
                sample=metrics(p.pid,kernel,psapi)
                if sample is None:raise RuntimeError("probe exited during idle")
                samples.append(dict(seconds=time.perf_counter()-idle_start,**sample));time.sleep(.05)
            seed=root.parent/(root.name+"-native-seed")
            seed.mkdir();shutil.copytree(root/"managed",seed/"managed");shutil.copyfile(root/"PANDORA-DISPOSABLE.txt",seed/"PANDORA-DISPOSABLE.txt")
            results=[]
            for phase in ["converged","edit","conflict","copy"]+["repeat"]*6:
                if phase in ["edit","conflict","copy"]:
                    p.stdin.write("prepare-"+phase+"\n");p.stdin.flush()
                    if not json.loads(p.stdout.readline()).get("prepared"):raise RuntimeError("workload did not prepare")
                before=metrics(p.pid,kernel,psapi);start=time.perf_counter()
                p.stdin.write(phase+"\n");p.stdin.flush();line=p.stdout.readline();out=json.loads(line)
                after=metrics(p.pid,kernel,psapi)
                io_delta=delta(before,after)
                # This offline probe has only managed-file I/O and these known
                # command/result pipes. Preserve raw counters as well.
                file_io=dict(logicalFileReadBytes=io_delta["logicalReadBytes"]-len((phase+os.linesep).encode()),logicalFileWriteBytes=io_delta["logicalWriteBytes"]-len(line.encode()))
                results.append(dict(**out,**io_delta,**file_io,wallSeconds=time.perf_counter()-start,workingSet=after["workingSet"],privateCommitted=after["privateCommitted"]))
            p.stdin.write("exit\n");p.stdin.flush();p.wait(timeout=15)
            if p.returncode:raise RuntimeError(f"probe exit {p.returncode}")
            return dict(count=n,exeSha256=hashlib.sha256(exe.read_bytes()).hexdigest(),fixture=str(root),cold=cold,idle=dict(workingSet=statistics.median(v["workingSet"] for v in samples),privateCommitted=statistics.median(v["privateCommitted"] for v in samples),**delta(samples[0],samples[-1]),sampleSeconds=samples[-1]["seconds"]-samples[0]["seconds"]),cycles=results)
        finally:
            if p.poll() is None:p.terminate();p.wait(timeout=15)


def native(exe,seed,root,seconds,legacy_policy=False):
    if not (seed/"PANDORA-DISPOSABLE.txt").read_text().startswith("PANDORA-4 synthetic fixture"):
        raise RuntimeError("not an owned fixture")
    if b"AURORA_DIAGNOSTIC_DATA_ROOT" not in exe.read_bytes():raise RuntimeError("binary lacks disposable-root support")
    root.mkdir();shutil.copytree(seed/"managed",root/"managed")
    if legacy_policy:
        policy_path=root/"managed/launcher/instance-sync.json"
        policy=json.loads(policy_path.read_text())
        if policy.pop("overrides",{}):raise RuntimeError("legacy fixture cannot discard nonempty overrides")
        policy_path.write_text(json.dumps(policy))
    env=os.environ.copy();env["AURORA_DIAGNOSTIC_DATA_ROOT"]=str(root/"managed");env["AURORA_DIAGNOSTIC_BOREALIS_TRACE"]="1"
    kernel,psapi=setup();samples=[]
    with (root/"stdout.txt").open("wb") as out,(root/"stderr.txt").open("wb") as err:
        p=subprocess.Popen([str(exe)],stdout=out,stderr=err,env=env)
        try:
            start=time.perf_counter()
            while p.poll() is None and time.perf_counter()-start<seconds:
                value=metrics(p.pid,kernel,psapi)
                if value:samples.append(dict(seconds=time.perf_counter()-start,**value))
                time.sleep(.05)
            if p.poll() is not None:raise RuntimeError(f"native exited {p.returncode}")
        finally:
            if p.poll() is None:p.terminate();p.wait(timeout=15)
    expected=str(root/"managed").replace("\\","/").lower()
    if "managed data root: "+expected not in (root/"stderr.txt").read_text(errors="replace").replace("\\","/").lower():raise RuntimeError("disposable root unconfirmed")
    idle=[s for s in samples if 8<=s["seconds"]<=55]
    tick=[s for s in samples if s["seconds"]>=58]
    return dict(exeSha256=hashlib.sha256(exe.read_bytes()).hexdigest(),samples=samples,tickWindow=dict(sampleSeconds=tick[-1]["seconds"]-tick[0]["seconds"],**delta(tick[0],tick[-1])),lastMemory={k:samples[-1][k] for k in ["workingSet","privateCommitted"]},idle=dict(workingSet=statistics.median(s["workingSet"] for s in idle),privateCommitted=statistics.median(s["privateCommitted"] for s in idle),sampleSeconds=idle[-1]["seconds"]-idle[0]["seconds"],**delta(idle[0],idle[-1])),wholeWindow=delta(samples[0],samples[-1]),termination="exact owned Popen handle")


def main():
    parser=argparse.ArgumentParser();parser.add_argument("mode",choices=["engine","native"]);parser.add_argument("exe",type=Path);parser.add_argument("evidence",type=Path);parser.add_argument("--label",required=True);parser.add_argument("--seed",type=Path);parser.add_argument("--seconds",type=float,default=72);parser.add_argument("--counts",type=int,nargs="+",default=[0,2,8,32,64]);parser.add_argument("--legacy-policy",action="store_true")
    args=parser.parse_args()
    if os.name!="nt" or not args.evidence.is_absolute() or not args.evidence.is_dir() or not args.evidence.name.startswith("aurora-pandora4-"):parser.error("existing explicit absolute aurora-pandora4-* evidence directory required")
    results=[]
    for n in args.counts:
        if n not in [0,2,8,32,64]:parser.error("unsupported pool size")
        root=args.evidence/f"pandora4-{args.label}-{n}"
        result=engine(args.exe,root,n) if args.mode=="engine" else native(args.exe,args.seed/f"pandora4-after-{n}-native-seed",root,args.seconds,args.legacy_policy)
        result["count"]=n;results.append(result)
        (args.evidence/f"{args.label}-results.json").write_text(json.dumps(results,indent=2))
        print(json.dumps(dict(count=n,idle=result["idle"])),flush=True)


if __name__=="__main__":main()
