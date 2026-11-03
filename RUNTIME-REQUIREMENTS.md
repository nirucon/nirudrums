# Runtime requirements — Linux x86_64

NIRUDRUMS 0.0.1 is built on Debian 13 (glibc) and tested on Debian 13 with REAPER 7.84.

The binary and VST3 bundle are dynamically linked. The editor and standalone window use X11/OpenGL. The standalone audio backend uses ALSA directly and can use JACK/PipeWire through NIH-plug's backend support.

## Required shared libraries

Observed with `ldd` on the shipped binaries:

- `libc.so.6`, `libm.so.6`, `libgcc_s.so.1`
- `libX11.so.6`, `libX11-xcb.so.1`, `libxcb.so.1`, `libXau.so.6`, `libXdmcp.so.6`
- `libGL.so.1`, `libGLX.so.0`, `libGLdispatch.so.0`
- `libasound.so.2` (standalone)
- `libbsd.so.0`, `libmd.so.0`

On Debian/Ubuntu the development packages used to build were `libasound2-dev`, `libjack-jackd2-dev`, `libx11-dev`, `libx11-xcb-dev`, `libxcb1-dev`, `libxcb-xkb-dev`, `libxkbcommon-dev`, `libxkbcommon-x11-dev`, `libgl1-mesa-dev`, `libegl1-mesa-dev`, `libxcursor-dev`, `libxrandr-dev`, `libxi-dev`, `libxinerama-dev`, `libwayland-dev` and `libfreetype6-dev`.

## Notes

- A graphical X11 session is required for the editor and standalone window. Headless rendering through REAPER works with a virtual X server when the host is configured for it.
- The standalone `--selftest` does not open a window and does not require an audio device.
- Arch and Void compatibility is expected to depend on their X11/OpenGL/ALSA libraries; those distributions were not tested for 0.0.1.
