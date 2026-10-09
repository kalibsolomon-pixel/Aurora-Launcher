import java.nio.file.*;
import java.util.*;
import jdk.jfr.*;

/** Generate an explicit deny-by-default configuration for this exact runtime. */
public final class BuildConfig {
    public static void main(String[] args) throws Exception {
        Map<String, String> enabled = new TreeMap<>();
        for (String name : List.of("ExecutionSample", "NativeMethodSample", "ObjectAllocationSample",
                "GarbageCollection", "GCPhasePause", "GCHeapSummary", "G1HeapSummary",
                "ThreadPark", "ThreadSleep", "JavaMonitorEnter", "JavaMonitorWait", "ThreadStart", "ThreadEnd",
                "ClassLoad", "ClassLoadingStatistics", "Compilation", "CompilerPhase", "CodeCacheStatistics",
                "CPULoad", "ThreadCPULoad", "FileRead", "FileWrite", "NativeLibrary")) {
            enabled.put("jdk." + name, name);
        }
        StringBuilder out = new StringBuilder("<?xml version=\"1.0\"?><configuration version=\"2.0\" label=\"Aurora P3 startup\" description=\"Local bounded startup investigation\" provider=\"Aurora research\">");
        for (EventType type : FlightRecorder.getFlightRecorder().getEventTypes()) {
            String name = type.getName(); boolean on = enabled.containsKey(name);
            out.append("<event name=\"").append(name).append("\"><setting name=\"enabled\">").append(on).append("</setting>");
            if (on) {
                for (SettingDescriptor setting : type.getSettingDescriptors()) {
                    String value = switch(setting.getName()) {
                        case "stackTrace" -> "true";
                        case "period" -> name.equals("jdk.NativeLibrary") ? "endChunk" :
                            (name.equals("jdk.ExecutionSample") || name.equals("jdk.NativeMethodSample") ? "20 ms" : "1 s");
                        case "threshold" -> "10 ms";
                        case "throttle" -> "50/s";
                        default -> null;
                    };
                    if (value != null) out.append("<setting name=\"").append(setting.getName()).append("\">").append(value).append("</setting>");
                }
            }
            out.append("</event>");
        }
        out.append("<event name=\"aurora.p3.Phase\"><setting name=\"enabled\">true</setting><setting name=\"stackTrace\">false</setting></event></configuration>");
        Files.writeString(Path.of(args[0]), out, StandardOpenOption.CREATE_NEW);
    }
}
