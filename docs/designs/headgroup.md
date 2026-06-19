THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.
THE ASSISTANT MAY NEVER USE THE ACRONYMS INDICES OR MARKERS IN THIS FILE AND MUST SPEAK IN PLAIN ENGLISH.

# actor modules

- window

### I/O

Window: RX<View<Color32>>, TX<PointStencil>

# actor threads

Window

# actor layout diagram:

Shadergroup ----> Window ----> Workgroup

# actor responsibilities desc:

The Window actor receives input from the user and displays the viewport.
It also converts these inputs into stencils which it sends to the workgroup, and
fills the viewport with the latest view from the shadergroup.

### Window

### Controller

### Sampler

### Notes

The headgroup only contains one actor; this is a forced design decision: splitting the headgroup into two or three actors would hurt responsiveness and introduce avoidable timing complexity. Egui threading details make it difficult to apply a troupe to alleviate timing issues.
