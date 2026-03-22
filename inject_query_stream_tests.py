import sys

file_path = "crates/force/src/api/rest/query_stream.rs"

with open(file_path, "r") as f:
    content = f.read()

test1 = """
    #[tokio::test]
    async fn test_query_stream_already_exhausted() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": true,
                "records": [
                    {"Id": "001", "Name": "A"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        let mut stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        let r1 = stream.next().await.must().must();
        assert_eq!(r1.name, "A");

        assert!(stream.next().await.must().is_none());
        assert!(stream.next().await.must().is_none());
    }

    #[tokio::test]
    async fn test_query_stream_not_done_no_url() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "totalSize": 1,
                "done": false,
                "records": [
                    {"Id": "001", "Name": "A"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        let mut stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        let r1 = stream.next().await.must().must();
        assert_eq!(r1.name, "A");

        assert!(stream.next().await.must().is_none());
    }

    #[tokio::test]
    async fn test_query_stream_into_stream_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/services/data/v60.0/query"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = builder().authenticate(auth).build().await.must();
        let stream = client
            .rest()
            .query_stream::<TestAccount>("SELECT Id, Name FROM Account");

        let results: Vec<_> = stream.into_stream().collect().await;
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }
}
"""

content = content.replace("}\n", "}\n" + test1)

# We actually only want to replace the last closing brace
with open(file_path, "r") as f:
    lines = f.readlines()

new_lines = lines[:-1]
new_lines.append(test1)

with open(file_path, "w") as f:
    f.writelines(new_lines)

print("Tests injected into query_stream.rs")
