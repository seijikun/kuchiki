use html5ever::{self, Attribute};
use html5ever::tree_builder::{TreeSink, NodeOrText, QuirksMode};
use std::borrow::Cow;
use string_cache::QualName;
use typed_arena::Arena;

use tree::Node;


pub fn parse<'a, F, I>(source: I, arena: &'a Arena<Node<'a>>, opts: ParseOpts<F>) -> &'a Node<'a>
                       where I: IntoIterator<Item=String>, F: FnMut(Cow<'static, str>) {
    let parser = Parser {
        arena: arena,
        document_node: Node::new_document(arena),
        on_parse_error: opts.on_parse_error,
    };
    let opts = html5ever::ParseOpts {
        tokenizer: opts.tokenizer,
        tree_builder: opts.tree_builder,
    };
    let parser = html5ever::parse_to(parser, source.into_iter(), opts);
    parser.document_node
}


pub struct ParseOpts<F> where F: FnMut(Cow<'static, str>) {
    pub tokenizer: html5ever::tokenizer::TokenizerOpts,
    pub tree_builder: html5ever::tree_builder::TreeBuilderOpts,
    pub on_parse_error: F,
}

struct IgnoreParseErrors;

impl<Args> FnOnce<Args> for IgnoreParseErrors {
    type Output = ();
    extern "rust-call" fn call_once(self, _args: Args) {}
}

impl<Args> FnMut<Args> for IgnoreParseErrors {
    extern "rust-call" fn call_mut(&mut self, _args: Args) {}
}

impl Default for ParseOpts<IgnoreParseErrors> {
    fn default() -> ParseOpts<IgnoreParseErrors> {
        ParseOpts {
            tokenizer: Default::default(),
            tree_builder: Default::default(),
            on_parse_error: IgnoreParseErrors,
        }
    }
}


struct Parser<'a, F> where F: FnMut(Cow<'static, str>) {
    arena: &'a Arena<Node<'a>>,
    document_node: &'a Node<'a>,
    on_parse_error: F,
}


impl<'a, F> TreeSink for Parser<'a, F> where F: FnMut(Cow<'static, str>) {
    type Handle = &'a Node<'a>;

    fn parse_error(&mut self, message: Cow<'static, str>) {
        (self.on_parse_error)(message);
    }

    fn get_document(&mut self) -> &'a Node<'a> {
        self.document_node
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.document_node.as_document().unwrap()._quirks_mode.set(mode)
    }

    fn same_node(&self, x: &'a Node<'a>, y: &'a Node<'a>) -> bool {
        x == y
    }

    fn elem_name(&self, target: &'a Node<'a>) -> QualName {
        target.as_element().unwrap().name.clone()
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> &'a Node<'a> {
        let attrs = attrs.into_iter().map(|Attribute { name, value }| (name, value));
        Node::new_element(name, attrs, self.arena)
    }

    fn create_comment(&mut self, text: String) -> &'a Node<'a> {
        Node::new_comment(text, self.arena)
    }

    fn append(&mut self, parent: &'a Node<'a>, child: NodeOrText<&'a Node<'a>>) {
        match child {
            NodeOrText::AppendNode(node) => parent.append(node),
            NodeOrText::AppendText(text) => {
                if let Some(last_child) = parent.last_child() {
                    if let Some(existing) = last_child.as_text() {
                        existing.borrow_mut().push_str(&text);
                        return
                    }
                }
                parent.append(Node::new_text(text, self.arena))
            }
        }
    }

    fn append_before_sibling(&mut self, sibling: &'a Node<'a>, child: NodeOrText<&'a Node<'a>>)
                             -> Result<(), NodeOrText<&'a Node<'a>>> {
        if sibling.parent().is_none() {
            return Err(child)
        }
        match child {
            NodeOrText::AppendNode(node) => sibling.insert_before(node),
            NodeOrText::AppendText(text) => {
                if let Some(previous_sibling) = sibling.previous_sibling() {
                    if let Some(existing) = previous_sibling.as_text() {
                        existing.borrow_mut().push_str(&text);
                        return Ok(())
                    }
                }
                sibling.insert_before(Node::new_text(text, self.arena))
            }
        }
        Ok(())
    }

    fn append_doctype_to_document(&mut self, name: String, public_id: String, system_id: String) {
        self.document_node.append(Node::new_doctype(name, public_id, system_id, self.arena))
    }

    fn add_attrs_if_missing(&mut self, target: &'a Node<'a>, attrs: Vec<Attribute>) {
        // FIXME: https://github.com/servo/html5ever/issues/121
        if let Some(element) = target.as_element() {
            let mut attributes = element.attributes.borrow_mut();
            for Attribute { name, value } in attrs {
                use std::collections::hash_map::Entry;
                match attributes.entry(name) {
                    Entry::Vacant(entry) => {
                        entry.insert(value);
                    }
                    Entry::Occupied(mut entry) => {
                        *entry.get_mut() = value;
                    }
                }
            }
        }
    }

    fn remove_from_parent(&mut self, target: &'a Node<'a>) {
        target.detach()
    }

    fn reparent_children(&mut self, node: &'a Node<'a>, new_parent: &'a Node<'a>) {
        // FIXME: Can this be done more effciently in rctree,
        // by moving the whole linked list of children at once?
        for child in node.children() {
            new_parent.append(child)
        }
    }

    fn mark_script_already_started(&mut self, _node: &'a Node<'a>) {
        // FIXME: Is this useful outside of a browser?
    }
}
