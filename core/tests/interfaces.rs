// Tests for graphql interfaces.

use pretty_assertions::assert_eq;

use graph::prelude::*;
use graph_graphql::prelude::object;
use graphql_parser::query as q;
use test_store::*;

// `entities` is `(entity, type)`.
fn insert_and_query(
    subgraph_id: &str,
    schema: &str,
    entities: Vec<(Entity, &str)>,
    query: &str,
) -> Result<QueryResult, StoreError> {
    let subgraph_id = SubgraphDeploymentId::new(subgraph_id).unwrap();
    create_test_subgraph(&subgraph_id, schema);

    let insert_ops = entities
        .into_iter()
        .map(|(data, entity_type)| EntityOperation::Set {
            key: EntityKey {
                subgraph_id: subgraph_id.clone(),
                entity_type: entity_type.to_owned(),
                entity_id: data["id"].clone().as_string().unwrap(),
            },
            data,
        });

    transact_entity_operations(
        &STORE,
        subgraph_id.clone(),
        GENESIS_PTR.clone(),
        insert_ops.collect::<Vec<_>>(),
    )?;

    let document = graphql_parser::parse_query(query).unwrap().into_static();
    let query = Query::new(
        STORE.api_schema(&subgraph_id).unwrap(),
        document,
        None,
        STORE.network_name(&subgraph_id).unwrap(),
    );
    Ok(execute_subgraph_query(query))
}

/// Extract the data from a `QueryResult`, and panic if it has errors
macro_rules! extract_data {
    ($result: expr) => {
        match $result.to_result() {
            Err(errors) => panic!(format!("Unexpected errors return for query: {:#?}", errors)),
            Ok(data) => data,
        }
    };
}

#[test]
fn one_interface_zero_entities() {
    let subgraph_id = "oneInterfaceZeroEntities";
    let schema = "interface Legged { legs: Int }
                  type Animal implements Legged @entity { id: ID!, legs: Int }";

    let query = "query { leggeds(first: 100) { legs } }";

    let res = insert_and_query(subgraph_id, schema, vec![], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(format!("{:?}", data), "Object({\"leggeds\": List([])})")
}

#[test]
fn one_interface_one_entity() {
    let subgraph_id = "oneInterfaceOneEntity";
    let schema = "interface Legged { legs: Int }
                  type Animal implements Legged @entity { id: ID!, legs: Int }";

    let entity = (
        Entity::from(vec![("id", Value::from("1")), ("legs", Value::from(3))]),
        "Animal",
    );

    // Collection query.
    let query = "query { leggeds(first: 100) { legs } }";
    let res = insert_and_query(subgraph_id, schema, vec![entity], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"leggeds\": List([Object({\"legs\": Int(Number(3))})])})"
    );

    // Query by ID.
    let query = "query { legged(id: \"1\") { legs } }";
    let res = insert_and_query(subgraph_id, schema, vec![], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"legged\": Object({\"legs\": Int(Number(3))})})",
    );
}

#[test]
fn one_interface_one_entity_typename() {
    let subgraph_id = "oneInterfaceOneEntityTypename";
    let schema = "interface Legged { legs: Int }
                  type Animal implements Legged @entity { id: ID!, legs: Int }";

    let entity = (
        Entity::from(vec![("id", Value::from("1")), ("legs", Value::from(3))]),
        "Animal",
    );

    let query = "query { leggeds(first: 100) { __typename } }";

    let res = insert_and_query(subgraph_id, schema, vec![entity], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"leggeds\": List([Object({\"__typename\": String(\"Animal\")})])})"
    )
}

#[test]
fn one_interface_multiple_entities() {
    let subgraph_id = "oneInterfaceMultipleEntities";
    let schema = "interface Legged { legs: Int }
                  type Animal implements Legged @entity { id: ID!, legs: Int }
                  type Furniture implements Legged @entity { id: ID!, legs: Int }
                  ";

    let animal = (
        Entity::from(vec![("id", Value::from("1")), ("legs", Value::from(3))]),
        "Animal",
    );
    let furniture = (
        Entity::from(vec![("id", Value::from("2")), ("legs", Value::from(4))]),
        "Furniture",
    );

    let query = "query { leggeds(first: 100, orderBy: legs) { legs } }";

    let res = insert_and_query(subgraph_id, schema, vec![animal, furniture], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"leggeds\": List([Object({\"legs\": Int(Number(3))}), Object({\"legs\": Int(Number(4))})])})"
    );

    // Test for support issue #32.
    let query = "query { legged(id: \"2\") { legs } }";
    let res = insert_and_query(subgraph_id, schema, vec![], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"legged\": Object({\"legs\": Int(Number(4))})})",
    );
}

#[test]
fn reference_interface() {
    let subgraph_id = "ReferenceInterface";
    let schema = "type Leg @entity { id: ID! }
                  interface Legged { leg: Leg }
                  type Animal implements Legged @entity { id: ID!, leg: Leg }";

    let query = "query { leggeds(first: 100) { leg { id } } }";

    let leg = (Entity::from(vec![("id", Value::from("1"))]), "Leg");
    let animal = (
        Entity::from(vec![("id", Value::from("1")), ("leg", Value::from("1"))]),
        "Animal",
    );

    let res = insert_and_query(subgraph_id, schema, vec![leg, animal], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"leggeds\": List([Object({\"leg\": Object({\"id\": String(\"1\")})})])})"
    )
}

#[test]
fn reference_interface_derived() {
    // Test the different ways in which interface implementations
    // can reference another entity
    let subgraph_id = "ReferenceInterfaceDerived";
    let schema = "
    type Transaction @entity {
        id: ID!,
        buyEvent: BuyEvent!,
        sellEvents: [SellEvent!]!,
        giftEvent: [GiftEvent!]! @derivedFrom(field: \"transaction\"),
    }

    interface Event {
        id: ID!,
        transaction: Transaction!
    }

    type BuyEvent implements Event @entity {
        id: ID!,
        # Derived, but only one buyEvent per Transaction
        transaction: Transaction! @derivedFrom(field: \"buyEvent\")
    }

    type SellEvent implements Event @entity {
        id: ID!
        # Derived, many sellEvents per Transaction
        transaction: Transaction! @derivedFrom(field: \"sellEvents\")
    }

    type GiftEvent implements Event @entity {
        id: ID!,
        # Store the transaction directly
        transaction: Transaction!
    }";

    let query = "query { events { id transaction { id } } }";

    let buy = (Entity::from(vec![("id", "buy".into())]), "BuyEvent");
    let sell1 = (Entity::from(vec![("id", "sell1".into())]), "SellEvent");
    let sell2 = (Entity::from(vec![("id", "sell2".into())]), "SellEvent");
    let gift = (
        Entity::from(vec![("id", "gift".into()), ("transaction", "txn".into())]),
        "GiftEvent",
    );
    let txn = (
        Entity::from(vec![
            ("id", "txn".into()),
            ("buyEvent", "buy".into()),
            ("sellEvents", vec!["sell1", "sell2"].into()),
        ]),
        "Transaction",
    );

    let entities = vec![buy, sell1, sell2, gift, txn];
    let res = insert_and_query(subgraph_id, schema, entities.clone(), query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"events\": List([\
            Object({\"id\": String(\"buy\"), \"transaction\": Object({\"id\": String(\"txn\")})}), \
            Object({\"id\": String(\"gift\"), \"transaction\": Object({\"id\": String(\"txn\")})}), \
            Object({\"id\": String(\"sell1\"), \"transaction\": Object({\"id\": String(\"txn\")})}), \
            Object({\"id\": String(\"sell2\"), \"transaction\": Object({\"id\": String(\"txn\")})})])})");
}

#[test]
fn follow_interface_reference_invalid() {
    let subgraph_id = "FollowInterfaceReferenceInvalid";
    let schema = "interface Legged { legs: Int! }
                  type Animal implements Legged @entity {
                    id: ID!
                    legs: Int!
                    parent: Legged
                  }";

    let query = "query { legged(id: \"child\") { parent { id } } }";

    let res = insert_and_query(subgraph_id, schema, vec![], query).unwrap();

    match &res.to_result().unwrap_err()[0] {
        QueryError::ExecutionError(QueryExecutionError::UnknownField(_, type_name, field_name)) => {
            assert_eq!(type_name, "Legged");
            assert_eq!(field_name, "parent");
        }
        e => panic!("error {} is not the expected one", e),
    }
}

#[test]
fn follow_interface_reference() {
    let subgraph_id = "FollowInterfaceReference";
    let schema = "interface Legged { id: ID!, legs: Int! }
                  type Animal implements Legged @entity {
                    id: ID!
                    legs: Int!
                    parent: Legged
                  }";

    let query = "query { legged(id: \"child\") { ... on Animal { parent { id } } } }";

    let parent = (
        Entity::from(vec![
            ("id", Value::from("parent")),
            ("legs", Value::from(4)),
            ("parent", Value::Null),
        ]),
        "Animal",
    );
    let child = (
        Entity::from(vec![
            ("id", Value::from("child")),
            ("legs", Value::from(3)),
            ("parent", Value::String("parent".into())),
        ]),
        "Animal",
    );

    let res = insert_and_query(subgraph_id, schema, vec![parent, child], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"legged\": Object({\"parent\": Object({\"id\": String(\"parent\")})})})"
    )
}

#[test]
fn conflicting_implementors_id() {
    let subgraph_id = "ConflictingImplementorsId";
    let schema = "interface Legged { legs: Int }
                  type Animal implements Legged @entity { id: ID!, legs: Int }
                  type Furniture implements Legged @entity { id: ID!, legs: Int }
                  ";

    let animal = (
        Entity::from(vec![("id", Value::from("1")), ("legs", Value::from(3))]),
        "Animal",
    );
    let furniture = (
        Entity::from(vec![("id", Value::from("1")), ("legs", Value::from(3))]),
        "Furniture",
    );

    let query = "query { leggeds(first: 100) { legs } }";

    let res = insert_and_query(subgraph_id, schema, vec![animal, furniture], query);

    let msg = res.unwrap_err().to_string();
    // We don't know in which order the two entities get inserted; the two
    // error messages only differ in who gets inserted first
    const EXPECTED1: &str =
        "tried to set entity of type `Furniture` with ID \"1\" but an entity of type `Animal`, \
         which has an interface in common with `Furniture`, exists with the same ID";
    const EXPECTED2: &str =
        "tried to set entity of type `Animal` with ID \"1\" but an entity of type `Furniture`, \
         which has an interface in common with `Animal`, exists with the same ID";

    assert!(msg == EXPECTED1 || msg == EXPECTED2);
}

#[test]
fn derived_interface_relationship() {
    let subgraph_id = "DerivedInterfaceRelationship";
    let schema = "interface ForestDweller { id: ID!, forest: Forest }
                  type Animal implements ForestDweller @entity { id: ID!, forest: Forest }
                  type Forest @entity { id: ID!, dwellers: [ForestDweller]! @derivedFrom(field: \"forest\") }
                  ";

    let forest = (Entity::from(vec![("id", Value::from("1"))]), "Forest");
    let animal = (
        Entity::from(vec![("id", Value::from("1")), ("forest", Value::from("1"))]),
        "Animal",
    );

    let query = "query { forests(first: 100) { dwellers(first: 100) { id } } }";

    let res = insert_and_query(subgraph_id, schema, vec![forest, animal], query).unwrap();
    let data = extract_data!(res);
    assert_eq!(
        data.unwrap().to_string(),
        "{forests: [{dwellers: [{id: \"1\"}]}]}"
    );
}

#[test]
fn two_interfaces() {
    let subgraph_id = "TwoInterfaces";
    let schema = "interface IFoo { foo: String! }
                  interface IBar { bar: Int! }

                  type A implements IFoo @entity { id: ID!, foo: String! }
                  type B implements IBar @entity { id: ID!, bar: Int! }

                  type AB implements IFoo & IBar @entity { id: ID!, foo: String!, bar: Int! }
                  ";

    let a = (
        Entity::from(vec![("id", Value::from("1")), ("foo", Value::from("bla"))]),
        "A",
    );
    let b = (
        Entity::from(vec![("id", Value::from("1")), ("bar", Value::from(100))]),
        "B",
    );
    let ab = (
        Entity::from(vec![
            ("id", Value::from("2")),
            ("foo", Value::from("ble")),
            ("bar", Value::from(200)),
        ]),
        "AB",
    );

    let query = "query {
                    ibars(first: 100, orderBy: bar) { bar }
                    ifoos(first: 100, orderBy: foo) { foo }
                }";
    let res = insert_and_query(subgraph_id, schema, vec![a, b, ab], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        "Object({\"ibars\": List([Object({\"bar\": Int(Number(100))}), Object({\"bar\": Int(Number(200))})]), \
                 \"ifoos\": List([Object({\"foo\": String(\"bla\")}), Object({\"foo\": String(\"ble\")})])})"
    );
}

#[test]
fn interface_non_inline_fragment() {
    let subgraph_id = "interfaceNonInlineFragment";
    let schema = "interface Legged { legs: Int }
                  type Animal implements Legged @entity { id: ID!, name: String, legs: Int }";

    let entity = (
        Entity::from(vec![
            ("id", Value::from("1")),
            ("name", Value::from("cow")),
            ("legs", Value::from(3)),
        ]),
        "Animal",
    );

    // Query only the fragment.
    let query = "query { leggeds { ...frag } } fragment frag on Animal { name }";
    let res = insert_and_query(subgraph_id, schema, vec![entity], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        r#"Object({"leggeds": List([Object({"name": String("cow")})])})"#
    );

    // Query the fragment and something else.
    let query = "query { leggeds { legs, ...frag } } fragment frag on Animal { name }";
    let res = insert_and_query(subgraph_id, schema, vec![], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        r#"Object({"leggeds": List([Object({"legs": Int(Number(3)), "name": String("cow")})])})"#,
    );
}

#[test]
fn interface_inline_fragment() {
    let subgraph_id = "interfaceInlineFragment";
    let schema = "interface Legged { legs: Int }
                  type Animal implements Legged @entity { id: ID!, name: String, legs: Int }
                  type Bird implements Legged @entity { id: ID!, airspeed: Int, legs: Int }";

    let animal = (
        Entity::from(vec![
            ("id", Value::from("1")),
            ("name", Value::from("cow")),
            ("legs", Value::from(4)),
        ]),
        "Animal",
    );
    let bird = (
        Entity::from(vec![
            ("id", Value::from("2")),
            ("airspeed", Value::from(24)),
            ("legs", Value::from(2)),
        ]),
        "Bird",
    );

    let query =
        "query { leggeds(orderBy: legs) { ... on Animal { name } ...on Bird { airspeed } } }";
    let res = insert_and_query(subgraph_id, schema, vec![animal, bird], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        format!("{:?}", data),
        r#"Object({"leggeds": List([Object({"airspeed": Int(Number(24))}), Object({"name": String("cow")})])})"#
    );
}

#[test]
fn interface_inline_fragment_with_subquery() {
    let subgraph_id = "InterfaceInlineFragmentWithSubquery";
    let schema = "
        interface Legged { legs: Int }
        type Parent @entity {
          id: ID!
        }
        type Animal implements Legged @entity {
          id: ID!
          name: String
          legs: Int
          parent: Parent
        }
        type Bird implements Legged @entity {
          id: ID!
          airspeed: Int
          legs: Int
          parent: Parent
        }
    ";

    let mama_cow = (
        Entity::from(vec![("id", Value::from("mama_cow"))]),
        "Parent",
    );
    let cow = (
        Entity::from(vec![
            ("id", Value::from("1")),
            ("name", Value::from("cow")),
            ("legs", Value::from(4)),
            ("parent", Value::from("mama_cow")),
        ]),
        "Animal",
    );

    let mama_bird = (
        Entity::from(vec![("id", Value::from("mama_bird"))]),
        "Parent",
    );
    let bird = (
        Entity::from(vec![
            ("id", Value::from("2")),
            ("airspeed", Value::from(5)),
            ("legs", Value::from(2)),
            ("parent", Value::from("mama_bird")),
        ]),
        "Bird",
    );

    let query = "query { leggeds(orderBy: legs) { legs ... on Bird { airspeed parent { id } } } }";
    let res = insert_and_query(
        subgraph_id,
        schema,
        vec![cow, mama_cow, bird, mama_bird],
        query,
    )
    .unwrap();
    let data = extract_data!(res).unwrap();

    assert_eq!(
        format!("{:?}", data),
        "Object({\
         \"leggeds\": List([\
         Object({\
         \"airspeed\": Int(Number(5)), \
         \"legs\": Int(Number(2)), \
         \"parent\": Object({\"id\": String(\"mama_bird\")})\
         }), \
         Object({\"legs\": Int(Number(4))})\
         ])\
         })"
    );
}

#[test]
fn invalid_fragment() {
    let subgraph_id = "InvalidFragment";
    let schema = "interface Legged { legs: Int! }
                  type Animal implements Legged @entity {
                    id: ID!
                    name: String!
                    legs: Int!
                    parent: Legged
                  }";

    let query = "query { legged(id: \"child\") { ...{ name } } }";

    let res = insert_and_query(subgraph_id, schema, vec![], query).unwrap();

    match &res.to_result().unwrap_err()[0] {
        QueryError::ExecutionError(QueryExecutionError::UnknownField(_, type_name, field_name)) => {
            assert_eq!(type_name, "Legged");
            assert_eq!(field_name, "name");
        }
        e => panic!("error {} is not the expected one", e),
    }
}

#[test]
fn alias() {
    let subgraph_id = "Alias";
    let schema = "interface Legged { id: ID!, legs: Int! }
                  type Animal implements Legged @entity {
                    id: ID!
                    legs: Int!
                    parent: Legged
                  }";

    let query = "query {
                    l: legged(id: \"child\") {
                        ... on Animal {
                            p: parent {
                                i: id,
                                t: __typename,
                                __typename
                            }
                        }
                    }
            }";

    let parent = (
        Entity::from(vec![
            ("id", Value::from("parent")),
            ("legs", Value::from(4)),
            ("parent", Value::Null),
        ]),
        "Animal",
    );
    let child = (
        Entity::from(vec![
            ("id", Value::from("child")),
            ("legs", Value::from(3)),
            ("parent", Value::String("parent".into())),
        ]),
        "Animal",
    );

    let res = insert_and_query(subgraph_id, schema, vec![parent, child], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            l: object! {
                p: object! {
                    i: "parent",
                    t: "Animal",
                    __typename: "Animal"
                }
            }
        }
    )
}

#[test]
fn fragments_dont_panic() {
    let subgraph_id = "FragmentsDontPanic";
    let schema = "
      type Parent @entity {
        id: ID!
        child: Child
      }

      type Child @entity {
        id: ID!
      }
    ";

    let query = "
        query {
            parents {
                ...on Parent {
                    child {
                        id
                    }
                }
                ...Frag
                child {
                    id
                }
            }
        }

        fragment Frag on Parent {
            child {
                id
            }
        }
    ";

    // The panic manifests if two parents exist.
    let parent = (
        entity!(
            id: "p",
            child: "c",
        ),
        "Parent",
    );
    let parent2 = (
        entity!(
            id: "p2",
            child: Value::Null,
        ),
        "Parent",
    );
    let child = (
        entity!(
            id:"c"
        ),
        "Child",
    );

    let res = insert_and_query(subgraph_id, schema, vec![parent, parent2, child], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            parents: vec![
                object! {
                    child: object! {
                        id: "c",
                    }
                },
                object! {
                    child: q::Value::Null
                }
            ]
        }
    )
}

// See issue #1816
#[test]
fn fragments_dont_duplicate_data() {
    let subgraph_id = "FragmentsDupe";
    let schema = "
      type Parent @entity {
        id: ID!
        children: [Child!]!
      }

      type Child @entity {
        id: ID!
      }
    ";

    let query = "
        query {
            parents {
                ...Frag
                children {
                    id
                }
            }
        }

        fragment Frag on Parent {
            children {
                id
            }
        }
    ";

    // This bug manifests if two parents exist.
    let parent = (
        entity!(
            id: "p",
            children: vec!["c"]
        ),
        "Parent",
    );
    let parent2 = (
        entity!(
            id: "b",
            children: Vec::<String>::new()
        ),
        "Parent",
    );
    let child = (
        entity!(
            id:"c"
        ),
        "Child",
    );

    let res = insert_and_query(subgraph_id, schema, vec![parent, parent2, child], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            parents: vec![
                object! {
                    children: Vec::<q::Value<'static, String>>::new()
                },
                object! {
                    children: vec![
                        object! {
                            id: "c",
                        }
                    ]
                }
            ]
        }
    )
}

// See also: e0d6da3e-60cf-41a5-b83c-b60a7a766d4a
#[test]
fn redundant_fields() {
    let subgraph_id = "RedundantFields";
    let schema = "interface Legged { id: ID!, parent: Legged }
                  type Animal implements Legged @entity {
                    id: ID!
                    parent: Legged
                  }";

    let query = "query {
                    leggeds {
                        parent { id }
                        ...on Animal {
                            parent { id }
                        }
                    }
            }";

    let parent = (
        entity!(
            id: "parent",
            parent: Value::Null,
        ),
        "Animal",
    );
    let child = (
        entity!(
            id: "child",
            parent: "parent",
        ),
        "Animal",
    );

    let res = insert_and_query(subgraph_id, schema, vec![parent, child], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            leggeds: vec![
                object! {
                    parent: object! {
                        id: "parent",
                    },
                },
                object! {
                    parent: q::Value::Null
                }
            ]
        }
    )
}

#[test]
fn fragments_merge_selections() {
    let subgraph_id = "FragmentsMergeSelections";
    let schema = "
      type Parent @entity {
        id: ID!
        children: [Child!]!
      }

      type Child @entity {
        id: ID!
        foo: Int!
      }
    ";

    let query = "
        query {
            parents {
                ...Frag
                children {
                    id
                }
            }
        }

        fragment Frag on Parent {
            children {
                foo
            }
        }
    ";

    let parent = (
        entity!(
            id: "p",
            children: vec!["c"]
        ),
        "Parent",
    );
    let child = (
        entity!(
            id: "c",
            foo: 1,
        ),
        "Child",
    );

    let res = insert_and_query(subgraph_id, schema, vec![parent, child], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            parents: vec![
                object! {
                    children: vec![
                        object! {
                            foo: 1,
                            id: "c",
                        }
                    ]
                }
            ]
        }
    )
}

#[test]
fn merge_fields_not_in_interface() {
    let subgraph_id = "MergeFieldsNotInInterface";
    let schema = "interface Iface { id: ID! }
                  type Animal implements Iface @entity {
                    id: ID!
                    human: Iface!
                  }
                  type Human implements Iface @entity {
                    id: ID!
                    animal: Iface!
                  }
                  ";

    let query = "query {
                    ifaces {
                        ...on Animal {
                            id
                            friend: human {
                              id
                            }
                        }
                        ...on Human {
                            id
                            friend: animal {
                              id
                            }
                        }
                    }
            }";

    let animal = (
        entity!(
            id: "cow",
            human: "fred",
        ),
        "Animal",
    );
    let human = (
        entity!(
            id: "fred",
            animal: "cow",
        ),
        "Human",
    );

    let res = insert_and_query(subgraph_id, schema, vec![animal, human], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            ifaces: vec![
                object! {
                    id: "cow",
                    friend: object! {
                        id: "fred",
                    },
                },
                object! {
                    id: "fred",
                    friend: object! {
                        id: "cow",
                    },
                },
            ]
        }
    )
}

#[test]
fn nested_interface_fragments() {
    let subgraph_id = "NestedInterfaceFragments";
    let schema = "interface I1face { id: ID!, foo1: Foo! }
                  interface I2face { id: ID!, foo2: Foo! }
                  interface I3face { id: ID!, foo3: Foo! }
                  type Foo @entity {
                      id: ID!
                  }
                  type One implements I1face @entity {
                    id: ID!
                    foo1: Foo!
                  }
                  type Two implements I1face & I2face @entity {
                    id: ID!
                    foo1: Foo!
                    foo2: Foo!
                  }
                  type Three implements I1face & I2face & I3face @entity {
                    id: ID!
                    foo1: Foo!
                    foo2: Foo!
                    foo3: Foo!
                  }";

    let query = "query {
                    i1Faces {
                        __typename
                        foo1 {
                            id
                        }
                        ...on I2face {
                            foo2 {
                                id
                            }
                        }
                        ...on I3face {
                            foo3 {
                                id
                            }
                        }
                    }
            }";

    let foo = (
        entity!(
            id: "foo",
        ),
        "Foo",
    );
    let one = (
        entity!(
            id: "1",
            foo1: "foo",
        ),
        "One",
    );
    let two = (
        entity!(
            id: "2",
            foo1: "foo",
            foo2: "foo",
        ),
        "Two",
    );
    let three = (
        entity!(
            id: "3",
            foo1: "foo",
            foo2: "foo",
            foo3: "foo"
        ),
        "Three",
    );

    let res = insert_and_query(subgraph_id, schema, vec![foo, one, two, three], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            i1Faces: vec![
                object! {
                    __typename: "One",
                    foo1: object! {
                        id: "foo",
                    },
                },
                object! {
                    __typename: "Two",
                    foo1: object! {
                        id: "foo",
                    },
                    foo2: object! {
                        id: "foo",
                    },
                },
                object! {
                    __typename: "Three",
                    foo1: object! {
                        id: "foo",
                    },
                    foo2: object! {
                        id: "foo",
                    },
                    foo3: object! {
                        id: "foo",
                    },
                },
            ]
        }
    )
}

#[test]
fn nested_interface_fragments_overlapping() {
    let subgraph_id = "NestedInterfaceFragmentsOverlapping";
    let schema = "interface I1face { id: ID!, foo1: Foo! }
                  interface I2face { id: ID!, foo1: Foo! }
                  type Foo @entity {
                      id: ID!
                  }
                  type One implements I1face @entity {
                    id: ID!
                    foo1: Foo!
                  }
                  type Two implements I1face & I2face @entity {
                    id: ID!
                    foo1: Foo!
                  }";

    let query = "query {
                    i1Faces {
                        __typename
                        ...on I2face {
                            foo1 {
                                id
                            }
                        }
                    }
            }";

    let foo = (
        entity!(
            id: "foo",
        ),
        "Foo",
    );
    let one = (
        entity!(
            id: "1",
            foo1: "foo",
        ),
        "One",
    );
    let two = (
        entity!(
            id: "2",
            foo1: "foo",
        ),
        "Two",
    );
    let res = insert_and_query(subgraph_id, schema, vec![foo, one, two], query).unwrap();

    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            i1Faces: vec![
                object! {
                    __typename: "One"
                },
                object! {
                    __typename: "Two",
                    foo1: object! {
                        id: "foo",
                    },
                },
            ]
        }
    );

    let query = "query {
        i1Faces {
            __typename
            foo1 {
                id
            }
            ...on I2face {
                foo1 {
                    id
                }
            }
        }
    }";

    let res = insert_and_query(subgraph_id, schema, vec![], query).unwrap();
    let data = extract_data!(res).unwrap();
    assert_eq!(
        data,
        object! {
            i1Faces: vec![
                object! {
                    __typename: "One",
                    foo1: object! {
                        id: "foo"
                    }
                },
                object! {
                    __typename: "Two",
                    foo1: object! {
                        id: "foo",
                    },
                },
            ]
        }
    );
}
