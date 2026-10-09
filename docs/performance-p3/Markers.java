package p3;
import java.lang.instrument.*;
import java.lang.management.ManagementFactory;
import java.lang.reflect.Field;
import java.nio.file.*;
import java.util.*;
import jdk.jfr.*;

/** Bootstrap-visible marker bridge; no ASM or game class dependencies. */
public final class Markers {
    static final long ORIGIN = System.nanoTime();
    static final long EPOCH = System.currentTimeMillis();
    static final long UPTIME = ManagementFactory.getRuntimeMXBean().getUptime();
    static Path output;
    static final List<String> rows = new ArrayList<>();
    static final Map<String,long[]> totals = new TreeMap<>();
    static final ThreadLocal<Map<String,ArrayDeque<Long>>> starts = ThreadLocal.withInitial(HashMap::new);
    static volatile boolean menu;
    static int menuFrames;
    static Field screen, overlay;

    public static void jnaCheck(Class<?> nativeClass) {
        try {
            var temp = nativeClass.getDeclaredMethod("getTempDir"); temp.setAccessible(true);
            var dispatch = nativeClass.getDeclaredField("jnidispatchPath"); dispatch.setAccessible(true);
            Path intended = output.resolve("jna").toRealPath();
            Path actual = ((java.io.File)temp.invoke(null)).toPath().toRealPath();
            Path loaded = Path.of((String)dispatch.get(null)).toRealPath();
            mark(actual.equals(intended) && loaded.getParent().equals(intended) ? "jna.isolation-confirmed" : "error.jna-isolation", 0);
        } catch (Exception failure) { mark("error.jna-probe", 0); }
    }

    @Name("aurora.p3.Phase") @jdk.jfr.Label("Aurora P3 boundary") @Enabled(false) @StackTrace(false)
    public static final class Phase extends Event {
        public String boundary;
        public int edge;
        public long elapsedNs;
    }

    public static void premain(String argument, Instrumentation instrumentation) throws Exception {
        output = Path.of(argument).toRealPath();
        if (!output.getParent().getFileName().toString().equals("trials")) throw new AssertionError("trial required");
        mark("agent.premain", 0);
        Runtime.getRuntime().addShutdownHook(new Thread(() -> save("markers-exit.json"), "P3-marker-shutdown"));
    }

    public static synchronized void mark(String key, int edge) {
        if (rows.size() >= 4096) throw new AssertionError("marker overflow");
        long elapsed = System.nanoTime() - ORIGIN;
        rows.add("{\"boundary\":\"" + key + "\",\"edge\":" + edge + ",\"ns\":" + elapsed + "}");
        Phase phase = new Phase(); phase.boundary = key; phase.edge = edge; phase.elapsedNs = elapsed; phase.commit();
    }

    public static void enter(String key) {
        if (menu) return;
        starts.get().computeIfAbsent(key, k -> new ArrayDeque<>()).push(System.nanoTime());
        if (!key.equals("mixin.transform")) mark(key, 1);
    }

    public static synchronized void exit(String key) {
        if (menu) return;
        ArrayDeque<Long> stack = starts.get().get(key);
        if (stack == null || stack.isEmpty()) { mark("error.unmatched", 0); return; }
        long elapsed = System.nanoTime() - stack.pop();
        long[] total = totals.computeIfAbsent(key, k -> new long[2]); total[0]++; total[1] += elapsed;
        if (!key.equals("mixin.transform")) mark(key, 2);
    }

    public static void frame(Object minecraft) {
        if (menu) return;
        try {
            if (screen == null) {
                screen = minecraft.getClass().getDeclaredField("field_1755"); screen.setAccessible(true);
                overlay = minecraft.getClass().getDeclaredField("field_18175"); overlay.setAccessible(true);
            }
            Object active = screen.get(minecraft);
            if (active != null && active.getClass().getName().equals("com.aurora.client.screen.AuroraTitleScreen") && overlay.get(minecraft) == null) {
                if (++menuFrames == 2) { mark("menu.usable", 0); menu = true; save("markers-menu.json"); }
            } else menuFrames = 0;
        } catch (ReflectiveOperationException failure) {
            mark("error.menu-field", 0); menu = true;
        }
    }

    static synchronized void save(String name) {
        try {
            StringBuilder sums = new StringBuilder();
            for (var row : totals.entrySet()) {
                if (!sums.isEmpty()) sums.append(',');
                sums.append('"').append(row.getKey()).append("\":{\"calls\":").append(row.getValue()[0])
                    .append(",\"inclusiveNs\":").append(row.getValue()[1]).append('}');
            }
            String json = "{\"originUnixMs\":"+EPOCH+",\"originUptimeMs\":"+UPTIME+
                ",\"savedUnixMs\":"+System.currentTimeMillis()+",\"savedElapsedNs\":"+(System.nanoTime()-ORIGIN)+
                ",\"menuReady\":"+menu+",\"records\":["+String.join(",",rows)+"],\"totals\":{"+sums+"}}";
            Files.writeString(output.resolve(name), json, StandardOpenOption.CREATE_NEW);
        } catch (Exception failure) { throw new AssertionError("marker write failed"); }
    }

}
