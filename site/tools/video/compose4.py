#!/usr/bin/env python3
"""Assemble the hero video from captured frames + composited overlays. Usage: compose4.py zh|en"""
import os, subprocess, sys
L = sys.argv[1]
S = "/private/tmp/claude-501/-Users-rico-Desktop-Workspace/3699eb4d-db4b-48d2-861e-5673289d1615/scratchpad/shifu-site"
REC, OV, OUT = f"{S}/rec-{L}", f"{S}/ov", f"{S}/r4/build-{L}"
os.makedirs(OUT, exist_ok=True)
END = 15.5
fs = sorted((float(f.split("_")[1]), f) for f in os.listdir(REC) if f.endswith(".png"))
fs = [x for x in fs if 0 <= x[0] <= END]
with open(f"{OUT}/frames.txt", "w") as fh:
    for i, (t, f) in enumerate(fs):
        nxt = fs[i + 1][0] if i + 1 < len(fs) else END
        fh.write(f"file '{REC}/{f}'\nduration {max(nxt - t, 0.001):.3f}\n")
    fh.write(f"file '{REC}/{fs[-1][1]}'\n")
# The isolated instance labels itself "Shifu 测试" (any non-production bundle id does); the patch restores the
# production label from an earlier capture of the same build. Nothing else in the frame is altered.
patch = f"{OUT}/brand.png"
subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", f"{S}/app/{L}/main.png", "-vf", "crop=100:24:100:6", patch], check=True)

def ov(name, t_in, t_out, fin=0.15, fout=0.25):
    return name, t_in, t_out, fin, fout

layers = [ov("ring", 6.6, 9.7, 0.2, 0.35), ov("banner", 4.95, 7.9, 0.01, 0.35), ov("cmdj", 6.3, 8.0), ov("enter", 7.95, 8.95)]
inputs = ["-f", "concat", "-safe", "0", "-i", f"{OUT}/frames.txt", "-loop", "1", "-i", patch]
for n, *_ in layers:
    inputs += ["-loop", "1", "-t", str(END), "-i", f"{OV}/{n}-{L}.png"]
fc = ["[0]fps=30,crop=1200:770:0:0,format=rgba[c0]", "[c0][1]overlay=100:6:shortest=1,scale=1440:924:flags=lanczos[b0]"]
prev = "b0"
for i, (n, a, z, fi, fo) in enumerate(layers):
    k = i + 2
    fc.append(f"[{k}]format=rgba,fade=in:st={a}:d={fi}:alpha=1,fade=out:st={z - fo}:d={fo}:alpha=1[o{i}]")
    x = "0"
    if n == "banner":  # slide in from the right edge
        x = f"if(lt(t\\,{a + 0.3})\\,(1-(t-{a})/0.3)*420\\,0)"
    fc.append(f"[{prev}][o{i}]overlay=x='{x}':y=0:enable='between(t\\,{a}\\,{z})'[b{i + 1}]")
    prev = f"b{i + 1}"
# seamless loop: the last 0.6 s cross-fades into the opening frame
fc.append(f"[{prev}]trim=0:{END},setpts=PTS-STARTPTS,split[m][f]")
fc.append("[f]trim=end_frame=1,loop=loop=24:size=1,setpts=N/30/TB[s]")
fc.append(f"[m][s]xfade=transition=fade:duration=0.6:offset={END - 0.6},format=yuv420p[v]")
subprocess.run(["ffmpeg", "-v", "error", "-y", *inputs, "-filter_complex", ";".join(fc), "-map", "[v]", "-r", "30",
                "-c:v", "libx264", "-crf", "12", "-preset", "medium", "-pix_fmt", "yuv420p", f"{OUT}/master.mp4"], check=True)
print("master done")
