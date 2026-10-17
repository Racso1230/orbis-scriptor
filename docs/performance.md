# Performance checks

## Map editor: large image plus many pins (Phase 6)

**Status: TODO: measure.** The implementation is done, but the numbers below must be recorded by hand on a real Windows machine and a real Linux machine (WebKitGTK, which is what Tauri uses there). They were not measured when this file was written.

### Scenario

- Background image: 8000 x 6000 px (generate one with `node scripts/make-test-map.mjs`).
- 500 pins on the map (dev builds: map toolbar, "Seed 500 pins (dev)").
- Release-like conditions matter: run `npm run tauri build` (or at least `tauri dev` with the DevTools closed) for the final numbers.

### Methodology

1. Create a world, then a new entity of type Map. Click "Import map image" and choose the generated 8000 x 6000 PNG.
2. Click "Seed 500 pins (dev)" once. Confirm the pin count by eye (a dense scatter across the image).
3. Click "Fit to screen". Measure the following with a window of about 1600 x 900:
   - **Import-to-visible time:** from choosing the file to the image being drawn (stopwatch or DevTools Performance recording).
   - **Pan:** drag the map continuously for 10 seconds. Record the average and the 1% low FPS.
   - **Wheel zoom:** zoom in and out across the full range for 10 seconds. Record average and 1% low FPS.
   - **Pin drag:** drag a single pin around for 5 seconds. Record average FPS.
4. How to measure FPS: open DevTools (right-click, Inspect, or F12 in `tauri dev`), Performance tab, record while performing the action, read the frames summary. On Linux, `WEBKIT_DISABLE_COMPOSITING_MODE=1` and `WEBKIT_DISABLE_DMABUF_RENDERER=1` can change results; note any env vars used.
5. Note memory use of the app process after the image has loaded (Task Manager / `ps`).
6. Repeat 3 times, record the median.

Pass criterion from the plan: stays "smooth" (target at least 30 FPS average with no multi-second stalls) on both platforms.

### Results

| Platform                      | Hardware | Import to visible | Pan FPS (avg / 1% low) | Zoom FPS (avg / 1% low) | Pin drag FPS  | Memory | Notes |
| ----------------------------- | -------- | ----------------- | ---------------------- | ----------------------- | ------------- | ------ | ----- |
| Windows 11 (WebView2)         | TODO     | TODO: measure     | TODO: measure          | TODO: measure           | TODO: measure | TODO   |       |
| Linux (WebKitGTK), distro TBD | TODO     | TODO: measure     | TODO: measure          | TODO: measure           | TODO: measure | TODO   |       |

### What is already in place to keep it fast

- The background image sits in its own Konva layer with `listening={false}`, so it is never hit-tested.
- Pins are in a separate layer; each pin is a memoised React component, so wheel zoom and panning do not re-render React at all.
- The stage transform (scale and position) is applied directly to the Konva stage and kept in a ref, not React state. Pins keep a constant on-screen size by setting an inverse scale directly on their Konva nodes during zoom.
- Shapes use `perfectDrawEnabled={false}`.

### If Linux is slow: next steps (from the plan)

Try these in order, measuring after each:

1. Konva layer caching for the pin layer (`layer.cache()` while panning/zooming, cleared when pins change).
2. Lower-resolution background while moving (swap to a downscaled copy during pan and zoom, full resolution when idle). The backend already generates thumbnails.
3. Tiling the background (only draw visible tiles).
