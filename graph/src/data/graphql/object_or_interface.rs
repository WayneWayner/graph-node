use crate::prelude::Schema;
use graphql_parser::schema as s;
use std::collections::BTreeMap;

use super::ObjectTypeExt;

#[derive(Copy, Clone, Debug)]
pub enum ObjectOrInterface<'a> {
    Object(&'a s::ObjectType<'static, String>),
    Interface(&'a s::InterfaceType<'static, String>),
}

impl<'a> From<&'a s::ObjectType<'static, String>> for ObjectOrInterface<'a> {
    fn from(object: &'a s::ObjectType<'static, String>) -> Self {
        ObjectOrInterface::Object(object)
    }
}

impl<'a> From<&'a s::InterfaceType<'static, String>> for ObjectOrInterface<'a> {
    fn from(interface: &'a s::InterfaceType<'static, String>) -> Self {
        ObjectOrInterface::Interface(interface)
    }
}

impl<'a> ObjectOrInterface<'a> {
    pub fn is_object(self) -> bool {
        match self {
            ObjectOrInterface::Object(_) => true,
            ObjectOrInterface::Interface(_) => false,
        }
    }

    pub fn is_interface(self) -> bool {
        match self {
            ObjectOrInterface::Object(_) => false,
            ObjectOrInterface::Interface(_) => true,
        }
    }

    pub fn name(self) -> &'a str {
        match self {
            ObjectOrInterface::Object(object) => &object.name,
            ObjectOrInterface::Interface(interface) => &interface.name,
        }
    }

    pub fn directives(self) -> &'a Vec<s::Directive<'static, String>> {
        match self {
            ObjectOrInterface::Object(object) => &object.directives,
            ObjectOrInterface::Interface(interface) => &interface.directives,
        }
    }

    pub fn fields(self) -> &'a Vec<s::Field<'static, String>> {
        match self {
            ObjectOrInterface::Object(object) => &object.fields,
            ObjectOrInterface::Interface(interface) => &interface.fields,
        }
    }

    pub fn field(&self, name: &String) -> Option<&s::Field<'static, String>> {
        self.fields().iter().find(|field| &field.name == name)
    }

    pub fn object_types(
        self,
        schema: &'a Schema,
    ) -> Option<Vec<&'a s::ObjectType<'static, String>>> {
        match self {
            ObjectOrInterface::Object(object) => Some(vec![object]),
            ObjectOrInterface::Interface(interface) => schema
                .types_for_interface()
                .get(&interface.name)
                .map(|object_types| object_types.iter().collect()),
        }
    }

    /// `typename` is the name of an object type. Matches if `self` is an object and has the same
    /// name, or if self is an interface implemented by `typename`.
    pub fn matches(
        self,
        typename: &str,
        types_for_interface: &BTreeMap<String, Vec<s::ObjectType<'static, String>>>,
    ) -> bool {
        match self {
            ObjectOrInterface::Object(o) => o.name == typename,
            ObjectOrInterface::Interface(i) => types_for_interface[&i.name]
                .iter()
                .any(|o| o.name == typename),
        }
    }

    pub fn is_meta(&self) -> bool {
        match self {
            ObjectOrInterface::Object(o) => o.is_meta(),
            ObjectOrInterface::Interface(i) => i.is_meta(),
        }
    }
}
