"""Bounded Java 21 HotSpot G1 summaries, including shrink-safe heap accounting.

Reuses P1 syntax/statistics; historical P1 files remain unchanged. A collection's
parenthesized capacity is the END capacity, not the capacity before collection.
"""
import re


def parse(path,start,duration,p1):
    pauses,used_after,committed_after,events,cpu=[],[],[],[],[]
    seen=set();cpu_seen=set();collector=None;collections=full=outside=0
    def require(ok,message):
        if not ok: raise p1.InvalidData(message)
    for line in path.read_text(encoding='utf-8').splitlines():
        if 'Using ' in line and p1.GC_TAG.search(line):
            found=line.split('Using ',1)[1].strip()
            require(found=='G1' and collector in (None,found),'GC parser supports only HotSpot G1')
            collector=found
        require('OutOfMemoryError' not in line,'OOM requires failed-run classification')
        if re.search(r'\[gc,cpu\s*\]',line) and 'GC(' in line:
            match=re.search(r'GC\((\d+)\) User=(\d+(?:\.\d+)?)s Sys=(\d+(?:\.\d+)?)s Real=(\d+(?:\.\d+)?)s$',line.strip())
            uptime=p1.UPTIME.search(line)
            require(match and uptime,'malformed G1 CPU record')
            end=p1.number(uptime[1]);key=(match[1],end)
            require(key not in cpu_seen,'duplicate GC CPU record');cpu_seen.add(key)
            if start<=end<=start+duration:cpu.append(p1.number(match[2])+p1.number(match[3]))
        if not p1.GC_TAG.search(line) or 'GC(' not in line or 'Pause ' not in line: continue
        match,uptime=p1.PAUSE.search(line.strip()),p1.UPTIME.search(line)
        require(match and uptime,'malformed G1 pause summary or missing uptime')
        end=p1.number(uptime[1]);pause=p1.number(match[3])*{'ms':1,'us':.001,'s':1000}[match[4]]
        beginning=end-pause/1000
        if beginning<start or end>start+duration:
            require(not(end>=start and beginning<=start+duration),'pause crosses measurement boundary')
            outside+=1;continue
        key=(match[1],re.split(r'\s+\d+(?:\.\d+)?[BKMG]->',match[2])[0])
        require(key not in seen,'duplicate GC pause summary');seen.add(key)
        heap=p1.HEAP.search(match[2])
        if 'Pause Young' in match[2] or 'Pause Full' in match[2]:
            require(heap,'collection pause lacks heap occupancy');collections+=1
        if heap:
            before,after,capacity=[p1.bytes_value(heap[i],heap[i+1]) for i in (1,3,5)]
            # GC may shrink from a capacity larger than the printed end capacity.
            require(after<=capacity,'post-GC used heap exceeds end capacity')
            used_after.append(after);committed_after.append(capacity)
        pauses.append(pause);events.append((beginning,end));full+='Pause Full' in match[2]
    require(collector is not None,'collector header missing')
    return {'collector':collector,'pause_ms':p1.distribution(pauses),'total_pause_ms':sum(pauses),
            'pause_wall_percent':sum(pauses)/(duration*10),'collection_count':collections,
            'collections_per_minute':collections*60/duration,'full_gc_count':full,
            'max_post_gc_used_bytes':max(used_after,default=None),
            'max_post_gc_committed_bytes':max(committed_after,default=None),
            'gc_logged_phase_cpu_seconds':sum(cpu) if cpu else None,
            'gc_logged_phase_cpu_percent_one_core':sum(cpu)/duration*100 if cpu else None,
            'gc_cpu_limit':'rounded process CPU during logged phases; not isolated GC worker CPU or total GC CPU',
            'outside_window_pause_count':outside,'oom_count':None},events
