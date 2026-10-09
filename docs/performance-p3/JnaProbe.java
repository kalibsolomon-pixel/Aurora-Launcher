import java.nio.file.*;
import java.lang.reflect.*;

/** Uses the exact installed JNA and Java; never visits a default cleanup root. */
public final class JnaProbe {
    public static void main(String[] args) throws Exception {
        Path intended = Path.of(args[0]).toRealPath();
        if (!Path.of(System.getProperty("jna.tmpdir")).toRealPath().equals(intended))
            throw new AssertionError("override missing");
        Class<?> nativeClass = Class.forName("com.sun.jna.Native");
        Method getTemp = nativeClass.getDeclaredMethod("getTempDir");
        getTemp.setAccessible(true);
        Path resolved = ((java.io.File)getTemp.invoke(null)).toPath().toRealPath();
        if (!resolved.equals(intended)) throw new AssertionError("wrong cleanup directory");
        Field dispatch = nativeClass.getDeclaredField("jnidispatchPath");
        dispatch.setAccessible(true);
        Path loaded = Path.of((String)dispatch.get(null)).toRealPath();
        if (!loaded.getParent().equals(intended)) throw new AssertionError("wrong extraction directory");
        if (Files.exists(intended.resolve("jna-p3-fixture.dll")) ||
            Files.exists(intended.resolve("jna-p3-fixture.dll.x")))
            throw new AssertionError("positive cleanup control failed");
        System.out.println("isolated-extraction-and-cleanup-ok");
    }
}
