set -e
S=/private/tmp/claude-501/-Users-rico-Desktop-Workspace/3699eb4d-db4b-48d2-861e-5673289d1615/scratchpad/shifu-site
for L in zh en; do
  B=$S/r4/build-$L; O=$S/r4/out; mkdir -p $O
  ffmpeg -v error -y -i $B/master.mp4 -c:v libx264 -preset veryslow -crf 19 -tune animation -pix_fmt yuv420p -movflags +faststart -an $O/hero-$L.mp4
  ffmpeg -v error -y -i $B/master.mp4 -c:v libvpx-vp9 -crf 30 -b:v 0 -row-mt 1 -deadline good -cpu-used 2 -pix_fmt yuv420p -an $O/hero-$L.webm
  ffmpeg -v error -y -ss 7.1 -i $B/master.mp4 -frames:v 1 $B/poster.png && cwebp -quiet -q 82 $B/poster.png -o $O/hero-$L-poster.webp
  # phone: just the Codex pane column, where the whole approve loop happens
  ffmpeg -v error -y -i $B/master.mp4 -vf "crop=570:924:870:0,scale=480:-2:flags=lanczos" -c:v libx264 -preset veryslow -crf 21 -tune animation -pix_fmt yuv420p -movflags +faststart -an $O/hero-m-$L.mp4
  ffmpeg -v error -y -i $B/master.mp4 -vf "crop=570:924:870:0,scale=480:-2:flags=lanczos" -c:v libvpx-vp9 -crf 33 -b:v 0 -row-mt 1 -deadline good -cpu-used 2 -an $O/hero-m-$L.webm
  ffmpeg -v error -y -ss 7.1 -i $O/hero-m-$L.mp4 -frames:v 1 $B/poster-m.png && cwebp -quiet -q 82 $B/poster-m.png -o $O/hero-m-$L-poster.webp
done
ls -la $S/r4/out
