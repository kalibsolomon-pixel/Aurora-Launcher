"""Explicit synthetic data, never a Minecraft capture or hardware benchmark."""
import csv
import json
from pathlib import Path
from quality import sha256


def create(root):
    root.mkdir(parents=True,exist_ok=True)
    with (root/'frames.csv').open('w',newline='') as f:
        writer=csv.writer(f);writer.writerow(['timestamp','pid'])
        writer.writerows((i/100,123) for i in range(201))
    (root/'gc.log').write_text('[0.000s][info][gc] Using G1\n[301.000s][info][gc] GC(0) Pause Young (Normal) (G1 Evacuation Pause) 64M->16M(128M) 5.000ms\n[301.500s][info][gc] GC(1) Pause Full (G1 Compaction Pause) 32M->16M(128M) 10.000ms\n')
    columns=['monotonic_ns','pid','creation_id','cpu_time_ns','rss_bytes','peak_rss_bytes',
             'system_available_bytes','system_total_bytes','system_busy_ns','system_total_ns']
    origin=10**15
    with (root/'process.csv').open('w',newline='') as f:
        writer=csv.writer(f);writer.writerow(columns)
        for i in range(5):
            writer.writerow([origin+i*500_000_000,123,'fixture-creation',i*750_000_000,
                             200_000_000,220_000_000,8*1024**3,16*1024**3,i*1_000_000_000,i*14_000_000_000])
    h='a'*64
    m={'schemaVersion':1,'runId':'synthetic-parser-1','workloadId':'synthetic-no-game',
       'configurationId':'current','blockId':'block-1','sampleKind':'synthetic','status':'complete',
       'startUtc':'2026-10-16T12:00:00Z','endUtc':'2026-10-16T12:00:02Z',
       'warmupSeconds':300,'measurementSeconds':2,'window':{'startSeconds':0},
       'process':{'pid':123,'creationId':'fixture-creation','originMonotonicNs':origin,'logicalProcessors':28},
       'runtime':{'vendor':'synthetic','version':'21.0.0','build':'fixture+1','architecture':'x64','manifestSha1':'a'*40,'executableSha256':h},
       'minecraft':{'version':'synthetic-no-game','loader':'synthetic','loaderVersion':'none','fabricApi':None},
       'mods':[],'graphics':{'width':1920,'height':1080,'refreshHz':240,'windowMode':'windowed','renderDistance':12,'simulationDistance':8,'optionsSha256':h},
       'scene':{'worldSha256':h,'routeSha256':h,'settingsSha256':h},
       'hardware':{'cpu':'synthetic','gpu':'synthetic','gpuDriver':'fixture','ramBytes':16*1024**3,'os':'synthetic','osBuild':'fixture'},
       'environment':{'power':'synthetic','acConnected':None,'thermals':None,'background':'synthetic','display':'synthetic','quietPreflightSha256':h},
       'capture':{'tool':'fixture-generator','version':'1','format':'normalized-timestamps-v1','units':'s','frameSemantic':'synthetic-presentation','toolSha256':h,'settingsSha256':h},
       'clocks':{'gcUptimeToRun':{'source0':300,'source1':302,'run0':0,'run1':2,'uncertaintyMs':1,'maxDriftPpm':100}},
       'jvmArguments':['-Xmx2048m'],'artifacts':{},'startupSeconds':{'jvm':None,'mainMenu':None,'worldLoad':None},
       'qualityPolicy':{'minimumWarmupSeconds':300,'minimumFrames':100,'boundaryToleranceSeconds':.05,'maxProcessIntervalSeconds':1},
       'dataQualityWarnings':[]}
    for role,file in [('frames','frames.csv'),('gc','gc.log'),('process','process.csv')]:
        m['artifacts'][role]={'path':file,'sha256':sha256(root/file)}
    (root/'run.json').write_text(json.dumps(m,indent=2))
    return m


if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('new_directory',type=Path)
    args=p.parse_args();args.new_directory.mkdir(parents=False,exist_ok=False);create(args.new_directory)
