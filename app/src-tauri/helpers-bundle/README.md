This directory is filled by `scripts/release/package.sh` before bundling a release.
It holds the signed `Shifu Computer Use.app` built from the same source, shipped as `Shifu.app/Contents/Resources/helpers/Shifu Computer Use.app`,
so that an in-app update can refresh an installed helper too. Ordinary local builds leave it empty; generated files are ignored.
