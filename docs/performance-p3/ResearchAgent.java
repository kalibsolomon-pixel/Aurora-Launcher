package p3;

import java.lang.instrument.*;
import java.lang.management.ManagementFactory;
import java.lang.reflect.Field;
import java.nio.file.*;
import java.security.ProtectionDomain;
import java.util.*;
import java.util.concurrent.*;
import org.objectweb.asm.*;
import jdk.jfr.*;

/** Passive startup markers. No game input, token, argument or environment capture. */
public final class ResearchAgent {
    public static void premain(String argument, Instrumentation instrumentation) throws Exception {
        Path root=Path.of(argument).toRealPath().getParent().getParent();
        instrumentation.appendToBootstrapClassLoaderSearch(new java.util.jar.JarFile(root.resolve("markers.jar").toFile()));
        Markers.premain(argument, instrumentation);
        instrumentation.addTransformer(new Transformer());
    }
    static final class Transformer implements ClassFileTransformer {
        @Override public byte[] transform(ClassLoader loader, String name, Class<?> type,
                ProtectionDomain domain, byte[] bytes) {
            if (name == null || name.startsWith("p3/") || name.startsWith("java/") || name.startsWith("jdk/")) return null;
            ClassReader reader = new ClassReader(bytes);
            boolean event = reader.getSuperName().equals("jdk/jfr/Event");
            boolean selected = name.equals("net/fabricmc/loader/impl/FabricLoaderImpl")
                || name.equals("net/fabricmc/loader/impl/launch/knot/Knot")
                || name.equals("org/spongepowered/asm/mixin/transformer/MixinTransformer")
                || name.equals("net/minecraft/class_310") || name.equals("net/minecraft/client/main/Main")
                || name.startsWith("com/aurora/client/") || !name.startsWith("net/minecraft/");
            if (!event && !selected) return null;
            ClassWriter writer = new ClassWriter(reader, ClassWriter.COMPUTE_MAXS);
            boolean[] changed = {event};
            reader.accept(new ClassVisitor(Opcodes.ASM9, writer) {
                boolean enabledAnnotation;
                @Override public AnnotationVisitor visitAnnotation(String desc, boolean visible) {
                    if (event && desc.equals("Ljdk/jfr/Enabled;")) {
                        enabledAnnotation = true;
                        AnnotationVisitor av = super.visitAnnotation(desc, visible); av.visit("value", false); av.visitEnd();
                        return null;
                    }
                    return super.visitAnnotation(desc, visible);
                }
                @Override public void visitEnd() {
                    if (event && !enabledAnnotation) { AnnotationVisitor av = super.visitAnnotation("Ljdk/jfr/Enabled;", true); av.visit("value", false); av.visitEnd(); }
                    super.visitEnd();
                }
                @Override public MethodVisitor visitMethod(int access, String method, String desc, String signature, String[] exceptions) {
                    MethodVisitor visitor = super.visitMethod(access, method, desc, signature, exceptions);
                    String key = null;
                    boolean frame = name.equals("net/minecraft/class_310") && method.equals("method_1523") && desc.equals("(Z)V");
                    boolean jna = name.equals("com/sun/jna/Native") && method.equals("<clinit>");
                    if (name.equals("net/fabricmc/loader/impl/FabricLoaderImpl") && Set.of("load","setup","freeze","invokeEntrypoints").contains(method)) key = "fabric."+method;
                    if (name.equals("net/fabricmc/loader/impl/launch/knot/Knot") && method.equals("init")) key = "fabric.knot-init";
                    if (name.equals("org/spongepowered/asm/mixin/transformer/MixinTransformer") && method.equals("transformClassBytes")) key = "mixin.transform";
                    if (name.equals("net/minecraft/class_310") && method.equals("<init>")) key = "minecraft.constructor";
                    if (name.equals("net/minecraft/client/main/Main") && method.equals("main")) key = "minecraft.main";
                    if (desc.equals("()V") && Set.of("onInitialize","onInitializeClient","onPreLaunch").contains(method)) key = "entrypoint."+name.replace('/','.')+"."+method;
                    if (key == null && !frame && !jna) return visitor;
                    changed[0] = true;
                    final String label = key;
                    return new MethodVisitor(Opcodes.ASM9, visitor) {
                        @Override public void visitCode() {
                            super.visitCode();
                            if (label != null) { super.visitLdcInsn(label); super.visitMethodInsn(Opcodes.INVOKESTATIC,"p3/Markers","enter","(Ljava/lang/String;)V",false); }
                        }
                        @Override public void visitInsn(int opcode) {
                            if (opcode >= Opcodes.IRETURN && opcode <= Opcodes.RETURN) {
                                if (label != null) { super.visitLdcInsn(label); super.visitMethodInsn(Opcodes.INVOKESTATIC,"p3/Markers","exit","(Ljava/lang/String;)V",false); }
                                if (frame) { super.visitVarInsn(Opcodes.ALOAD,0); super.visitMethodInsn(Opcodes.INVOKESTATIC,"p3/Markers","frame","(Ljava/lang/Object;)V",false); }
                                if (jna) { super.visitLdcInsn(Type.getObjectType(name)); super.visitMethodInsn(Opcodes.INVOKESTATIC,"p3/Markers","jnaCheck","(Ljava/lang/Class;)V",false); }
                            }
                            super.visitInsn(opcode);
                        }
                    };
                }
            }, 0);
            return changed[0] ? writer.toByteArray() : null;
        }
    }
}
