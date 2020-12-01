use graphql_parser::{query as q, schema as s};
use std::collections::HashMap;

use crate::execution::ExecutionContext;
use graph::data::graphql::{ext::DocumentExt, ObjectOrInterface};
use graph::prelude::{QueryExecutionError, StoreEventStreamBox};

/// A GraphQL resolver that can resolve entities, enum values, scalar types and interfaces/unions.
pub trait Resolver: Sized + Send + Sync + 'static {
    const CACHEABLE: bool;

    /// Prepare for executing a query by prefetching as much data as possible
    fn prefetch(
        &self,
        ctx: &ExecutionContext<Self>,
        selection_set: &q::SelectionSet<'static, String>,
    ) -> Result<Option<q::Value<'static, String>>, Vec<QueryExecutionError>>;

    /// Resolves list of objects, `prefetched_objects` is `Some` if the parent already calculated the value.
    fn resolve_objects(
        &self,
        prefetched_objects: Option<q::Value<'static, String>>,
        field: &q::Field<'static, String>,
        field_definition: &s::Field<'static, String>,
        object_type: ObjectOrInterface<'_>,
        arguments: &HashMap<&String, q::Value<'static, String>>,
    ) -> Result<q::Value<'static, String>, QueryExecutionError>;

    /// Resolves an object, `prefetched_object` is `Some` if the parent already calculated the value.
    fn resolve_object(
        &self,
        prefetched_object: Option<q::Value<'static, String>>,
        field: &q::Field<'static, String>,
        field_definition: &s::Field<'static, String>,
        object_type: ObjectOrInterface<'_>,
        arguments: &HashMap<&String, q::Value<'static, String>>,
    ) -> Result<q::Value<'static, String>, QueryExecutionError>;

    /// Resolves an enum value for a given enum type.
    fn resolve_enum_value(
        &self,
        _field: &q::Field<'static, String>,
        _enum_type: &s::EnumType<'static, String>,
        value: Option<q::Value<'static, String>>,
    ) -> Result<q::Value<'static, String>, QueryExecutionError> {
        Ok(value.unwrap_or(q::Value::Null))
    }

    /// Resolves a scalar value for a given scalar type.
    fn resolve_scalar_value(
        &self,
        _parent_object_type: &s::ObjectType<'static, String>,
        _field: &q::Field<'static, String>,
        _scalar_type: &s::ScalarType<'static, String>,
        value: Option<q::Value<'static, String>>,
        _argument_values: &HashMap<&String, q::Value<'static, String>>,
    ) -> Result<q::Value<'static, String>, QueryExecutionError> {
        // This code is duplicated.
        // See also c2112309-44fd-4a84-92a0-5a651e6ed548
        Ok(value.unwrap_or(q::Value::Null))
    }

    /// Resolves a list of enum values for a given enum type.
    fn resolve_enum_values(
        &self,
        _field: &q::Field<'static, String>,
        _enum_type: &s::EnumType<'static, String>,
        value: Option<q::Value<'static, String>>,
    ) -> Result<q::Value<'static, String>, Vec<QueryExecutionError>> {
        Ok(value.unwrap_or(q::Value::Null))
    }

    /// Resolves a list of scalar values for a given list type.
    fn resolve_scalar_values(
        &self,
        _field: &q::Field<'static, String>,
        _scalar_type: &s::ScalarType<'static, String>,
        value: Option<q::Value<'static, String>>,
    ) -> Result<q::Value<'static, String>, Vec<QueryExecutionError>> {
        Ok(value.unwrap_or(q::Value::Null))
    }

    // Resolves an abstract type into the specific type of an object.
    fn resolve_abstract_type<'a>(
        &self,
        schema: &'a s::Document<'static, String>,
        _abstract_type: &s::TypeDefinition<'static, String>,
        object_value: &q::Value<'static, String>,
    ) -> Option<&'a s::ObjectType<'static, String>> {
        let concrete_type_name = match object_value {
            // All objects contain `__typename`
            q::Value::Object(data) => match &data["__typename"] {
                q::Value::String(name) => name.clone(),
                _ => unreachable!("__typename must be a string"),
            },
            _ => unreachable!("abstract type value must be an object"),
        };

        // A name returned in a `__typename` must exist in the schema.
        match schema.get_named_type(&concrete_type_name).unwrap() {
            s::TypeDefinition::Object(object) => Some(object),
            _ => unreachable!("only objects may implement interfaces"),
        }
    }

    // Resolves a change stream for a given field.
    fn resolve_field_stream<'a, 'b>(
        &self,
        _schema: &'a s::Document<'static, String>,
        _object_type: &'a s::ObjectType<'static, String>,
        _field: &'b q::Field<'static, String>,
    ) -> Result<StoreEventStreamBox, QueryExecutionError> {
        Err(QueryExecutionError::NotSupported(String::from(
            "Resolving field streams is not supported by this resolver",
        )))
    }
}
