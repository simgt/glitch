Features:
- [x] Remove direct use of gstreamer elements in rendering, necessary to decouple the tracer
- [x] Move tracer to a separate crate and build as a shared library
- [x] Display pads
- [x] Attach edges to pads
- [x] Replace bin's default toggle by a click on the whole box
- [x] Use #[derive(Bundle)] to clean up the components
- [x] Add inspector for elements (name, properties, state, factory name, etc.)
- [x] Allow selecting pads
- [ ] Display the content of pads, live
- [ ] Display caps
- [ ] Allow saving and reloading sessions
- [ ] Add a timeline that allows to revert to a previous state
- [ ] Allow navigating the graph with the keyboard
- [ ] Add IP and port properties to the tracer

Bugs and code improvements:
- [x] Fix nodes margins when zooming
- [x] Make the nodes sorting order in a layer stable
- [x] Layers should be stored as long as the topology doesn't change
- [x] Pads and Elements are not being spawned as bundles
- [x] Add a Node empty component instead of relying on ElementState as marker
- [x] Add a Port marker to the Pad bundle
- [x] Rename references to Element to Node and Pad to Port in the drawing code
- [x] Abstract away gstreamer from the communication layer
- [ ] Relayout shouldn't when translating the view
- [ ] Store nodes sizes and positions in ctx.memory()
