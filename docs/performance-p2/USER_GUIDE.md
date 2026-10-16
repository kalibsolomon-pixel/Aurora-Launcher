# Performance settings

Open **Settings → Minecraft & Java → Open instance Settings → Performance**.
Changes belong to that instance and apply on its next game launch after Save.

Efficient offers a smaller conservative memory target. Balanced uses conservative
advice with Java defaults. Performance currently offers the same advice as Balanced:
there are no real Minecraft measurements supporting different tuning yet. Custom
keeps manual access to supported memory and advanced options. Existing Configuration
preserves the established policy for older settings and is also the fresh default.

Choosing a mode keeps your heap and custom text. The saved mode/maximum remain
visible while you edit an unsaved selection. The recommendation is separate; **Use
… MiB in draft** refreshes it and changes only the draft. Save changes is the actual
native write. An error leaves saved settings unchanged and keeps your draft for retry.
Unknown/low-memory hardware may provide no advice; manual settings remain available.

Maximum heap is measured in MiB. Java, native libraries, graphics, the launcher and
other apps need memory outside that heap. A larger heap can increase system pressure
and does not guarantee more FPS. Advice reserves headroom but cannot know your
workload's live set or future graphics/background demand; review it before applying.
The initial heap remains Java-managed. There is no supported minimum-heap control.

Raw JVM arguments stay accessible in **Java → Additional JVM arguments**. Double
quotes group spaces. The four modes support ordinary user system properties and
assertion switches; heap/path/entrypoint/indirection and unvalidated GC/VM options
are rejected. Duplicate properties keep their order, with the final value effective.
No text is removed automatically. If an existing argument is not supported by a
mode, retain Existing Configuration or revise it deliberately. Never put credentials
in custom properties or ordinary configuration.

Before saving, Undo memory recommendation or Discard restores the prior draft/saved
setting. After saving, change the maximum manually and Save to restore a previous
value. Selecting Existing Configuration restores its validation policy without
inventing or restoring a different heap/argument list. No automatic custom-text
replacement, instance migration into a profile, system Java/power change or update
occurs when selecting a mode.

FPS, frame-time consistency, GC pauses, CPU use and startup improvements are
**unverified**. Performance is a configuration mode, not a performance promise.
