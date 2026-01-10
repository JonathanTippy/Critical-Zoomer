
// highlighting bools:
// filament, node, node-tree, edge

pub(crate) struct Highlighting {
    data: u8
}

impl Highlighting {


    fn filament(&self) -> bool {
        (self.data && 1).into()
    }
}