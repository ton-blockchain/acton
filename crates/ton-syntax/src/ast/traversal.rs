use tree_sitter::{Node, TreeCursor};

pub struct PreorderTraverse<'tree> {
    cursor: Option<TreeCursor<'tree>>,
}

impl<'tree> PreorderTraverse<'tree> {
    #[must_use]
    pub const fn new(c: TreeCursor<'tree>) -> Self {
        PreorderTraverse { cursor: Some(c) }
    }
}

impl<'tree> Iterator for PreorderTraverse<'tree> {
    type Item = Node<'tree>;

    fn next(&mut self) -> Option<Self::Item> {
        let c = match self.cursor.as_mut() {
            None => {
                return None;
            }
            Some(c) => c,
        };

        // We will always return the node we were on at the start;
        // the node we traverse to will either be returned on the next iteration,
        // or will be back to the root node, at which point we'll clear out
        // the reference to the cursor
        let node = c.node();

        // First, try to go to a child or a sibling; if either succeed, this will be the
        // first time we touch that node, so it'll be the next starting node
        if c.goto_first_child() || c.goto_next_sibling() {
            return Some(node);
        }

        loop {
            // If we can't go to the parent, then that means we've reached the root, and our
            // iterator will be done in the next iteration
            if !c.goto_parent() {
                self.cursor = None;
                break;
            }

            // If we get to a sibling, then this will be the first time we touch that node,
            // so it'll be the next starting node
            if c.goto_next_sibling() {
                break;
            }
        }

        Some(node)
    }
}
