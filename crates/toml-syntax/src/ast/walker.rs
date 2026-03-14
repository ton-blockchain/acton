use crate::ast::expressions::{
    Array, BareKey, BooleanLit, DottedKey, FloatLit, InlineTable, IntegerLit, Key, LocalDate,
    LocalDateTime, LocalTime, OffsetDateTime, Pair, QuotedKey, StringLit, Value,
};
use crate::ast::node::SourceFile;
use crate::ast::top_level::{Document, Table, TableArrayElement, TopLevel};

pub trait Walker<'tree> {
    type Result;

    fn default_result(&self) -> Self::Result;

    fn visit_source_file(&mut self, source_file: &'tree SourceFile) -> Self::Result {
        self.walk_source_file(source_file)
    }

    fn visit_top_level(&mut self, top_level: &TopLevel<'tree>) -> Self::Result {
        self.walk_top_level(top_level)
    }

    fn visit_key(&mut self, key: &Key<'tree>) -> Self::Result {
        self.walk_key(key)
    }

    fn visit_value(&mut self, value: &Value<'tree>) -> Self::Result {
        self.walk_value(value)
    }

    fn walk_source_file(&mut self, file: &'tree SourceFile) -> Self::Result {
        if let Some(document) = file.document() {
            self.walk_document(&document);
        }
        self.default_result()
    }

    fn walk_document(&mut self, node: &Document<'tree>) -> Self::Result {
        for item in node.items() {
            self.visit_top_level(&item);
        }
        self.default_result()
    }

    fn walk_top_level(&mut self, node: &TopLevel<'tree>) -> Self::Result {
        match node {
            TopLevel::Pair(p) => self.walk_pair(p),
            TopLevel::Table(t) => self.walk_table(t),
            TopLevel::TableArrayElement(t) => self.walk_table_array_element(t),
            TopLevel::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_table(&mut self, node: &Table<'tree>) -> Self::Result {
        if let Some(key) = node.key() {
            self.visit_key(&key);
        }
        for pair in node.pairs() {
            self.walk_pair(&pair);
        }
        self.default_result()
    }

    fn walk_table_array_element(&mut self, node: &TableArrayElement<'tree>) -> Self::Result {
        if let Some(key) = node.key() {
            self.visit_key(&key);
        }
        for pair in node.pairs() {
            self.walk_pair(&pair);
        }
        self.default_result()
    }

    fn walk_pair(&mut self, node: &Pair<'tree>) -> Self::Result {
        if let Some(key) = node.key() {
            self.visit_key(&key);
        }
        if let Some(value) = node.value() {
            self.visit_value(&value);
        }
        self.default_result()
    }

    fn walk_key(&mut self, node: &Key<'tree>) -> Self::Result {
        match node {
            Key::Bare(k) => self.walk_bare_key(k),
            Key::Quoted(k) => self.walk_quoted_key(k),
            Key::Dotted(k) => self.walk_dotted_key(k),
            Key::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_value(&mut self, node: &Value<'tree>) -> Self::Result {
        match node {
            Value::String(v) => self.walk_string_lit(v),
            Value::Integer(v) => self.walk_integer_lit(v),
            Value::Float(v) => self.walk_float_lit(v),
            Value::Boolean(v) => self.walk_boolean_lit(v),
            Value::OffsetDateTime(v) => self.walk_offset_date_time(v),
            Value::LocalDateTime(v) => self.walk_local_date_time(v),
            Value::LocalDate(v) => self.walk_local_date(v),
            Value::LocalTime(v) => self.walk_local_time(v),
            Value::Array(v) => self.walk_array(v),
            Value::InlineTable(v) => self.walk_inline_table(v),
            Value::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_dotted_key(&mut self, node: &DottedKey<'tree>) -> Self::Result {
        for part in node.parts() {
            self.visit_key(&part);
        }
        self.default_result()
    }

    fn walk_array(&mut self, node: &Array<'tree>) -> Self::Result {
        for value in node.values() {
            self.visit_value(&value);
        }
        self.default_result()
    }

    fn walk_inline_table(&mut self, node: &InlineTable<'tree>) -> Self::Result {
        for pair in node.pairs() {
            self.walk_pair(&pair);
        }
        self.default_result()
    }

    fn walk_bare_key(&mut self, _node: &BareKey<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_quoted_key(&mut self, _node: &QuotedKey<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_string_lit(&mut self, _node: &StringLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_integer_lit(&mut self, _node: &IntegerLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_float_lit(&mut self, _node: &FloatLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_boolean_lit(&mut self, _node: &BooleanLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_offset_date_time(&mut self, _node: &OffsetDateTime<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_local_date_time(&mut self, _node: &LocalDateTime<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_local_date(&mut self, _node: &LocalDate<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_local_time(&mut self, _node: &LocalTime<'tree>) -> Self::Result {
        self.default_result()
    }
}
