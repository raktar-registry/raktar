use async_graphql::{value, Name, Request, Value, Variables};
use raktar::graphql::schema::build_schema;
use raktar::repository::DynRepository;
use std::collections::HashSet;
use std::sync::Arc;

use crate::common::graphql::build_request;
use crate::common::setup::build_repository;

#[tokio::test]
async fn test_token_generation() {
    let repository = Arc::new(build_repository().await) as DynRepository;
    let schema = build_schema(repository);

    let request = build_generate_token_request(0, "test token");
    let response = schema.execute(request).await;
    assert_eq!(response.errors.len(), 0);

    let actual_name = extract_data(&response.data, &["generateToken", "token", "name"]);
    let expected_name = Value::String("test token".to_string());
    assert_eq!(actual_name, expected_name);

    let key = extract_data(&response.data, &["generateToken", "key"]);
    if let Value::String(k) = key {
        assert_eq!(k.len(), 32);
    } else {
        panic!("the key is not a string");
    }
}

#[tokio::test]
async fn test_my_tokens() {
    let repository = Arc::new(build_repository().await) as DynRepository;
    let schema = build_schema(repository);

    // We create a new token for user 10
    let request = build_generate_token_request(10, "test token");
    let response = schema.execute(request).await;
    assert_eq!(response.errors.len(), 0);

    // We create a new token with the same name for user 11
    let request = build_generate_token_request(11, "test token");
    let response = schema.execute(request).await;
    assert_eq!(response.errors.len(), 0);

    // For user 11, we create another token
    let request = build_generate_token_request(11, "test token 2");
    let response = schema.execute(request).await;
    assert_eq!(response.errors.len(), 0);

    // We get the tokens for user 11
    let response = schema.execute(build_my_tokens_request(11)).await;
    assert_eq!(response.errors.len(), 0);

    // There should be two tokens
    let tokens_data = extract_data(&response.data, &["myTokens"]);
    if let Value::List(tokens) = tokens_data {
        let actual: HashSet<String> = tokens
            .iter()
            .map(|t| extract_data(t, &["name"]).to_string())
            .collect();
        let mut expected = HashSet::new();
        expected.insert("\"test token\"".to_string());
        expected.insert("\"test token 2\"".to_string());
        assert_eq!(actual, expected);
    } else {
        panic!("tokens is not a list");
    }
}

#[tokio::test]
async fn test_delete_token() {
    let repository = Arc::new(build_repository().await) as DynRepository;
    let schema = build_schema(repository);

    let request = build_generate_token_request(20, "test token");
    let response = schema.execute(request).await;
    assert_eq!(response.errors.len(), 0);

    let request = build_generate_token_request(20, "test token 2");
    let response = schema.execute(request).await;
    assert_eq!(response.errors.len(), 0);
    let id = match extract_data(&response.data, &["generateToken", "id"]) {
        Value::String(id) => id,
        _ => panic!("id is not a string"),
    };

    let request = build_delete_token_request(20, id);
    let response = schema.execute(request).await;
    assert_eq!(response.errors.len(), 0);

    let response = schema.execute(build_my_tokens_request(20)).await;
    assert_eq!(response.errors.len(), 0);

    // There should be one token after the delete
    let tokens_data = extract_data(&response.data, &["myTokens"]);
    if let Value::List(tokens) = tokens_data {
        assert_eq!(tokens.len(), 1);
    } else {
        panic!("tokens is not a list");
    }
}

fn extract_data(data: &Value, path: &[&str]) -> Value {
    let mut actual = data.clone();
    for p in path {
        if let Value::Object(mut obj) = actual {
            let key = Name::new(p);
            actual = obj.remove(&key).expect("key to exist");
        } else {
            panic!("value at {} is not an object", p);
        }
    }

    actual
}

fn build_generate_token_request(user_id: u32, name: &str) -> Request {
    let mutation = r#"
    mutation GenerateToken($name: String!) {
        generateToken(name: $name) {
            id
            token {
                id
                userId
                name
            }
            key
        }
    }
    "#;
    let variables = Variables::from_value(value!({ "name": name }));

    build_request(mutation, user_id).variables(variables)
}

fn build_delete_token_request(user_id: u32, token_id: String) -> Request {
    let mutation = r#"
    mutation DeleteToken($tokenId: String!) {
      deleteToken(tokenId: $tokenId) {
        id
      }
    }
    "#;
    let variables = Variables::from_value(value!({ "tokenId": token_id }));

    build_request(mutation, user_id).variables(variables)
}

fn build_my_tokens_request(user_id: u32) -> Request {
    let query = r#"
    query {
      myTokens {
        name
      }
    }"#;
    build_request(query, user_id)
}
