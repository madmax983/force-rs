//! Apex test execution endpoints for the Tooling API.
//!
//! Provides types and methods for running Apex tests both synchronously and
//! asynchronously via the Salesforce Tooling API.
//!
//! - **Synchronous**: `POST /tooling/runTestsSynchronous` blocks until all
//!   tests complete and returns detailed results including pass/fail status
//!   and code coverage.
//! - **Asynchronous**: `POST /tooling/runTestsAsynchronous` returns immediately
//!   with a test run ID that can be polled via `ApexTestRunResult`.

use serde::{Deserialize, Serialize};

/// Request to run Apex tests.
///
/// Specifies which test classes and methods to execute, along with an optional
/// maximum failure threshold.
///
/// # Examples
///
/// ```
/// use force::api::tooling::run_tests::{RunTestsRequest, TestItem};
///
/// let request = RunTestsRequest {
///     tests: vec![
///         TestItem {
///             class_id: "01pxx000000ABCDE".to_string(),
///             test_methods: Some(vec!["testInsert".to_string()]),
///         },
///     ],
///     max_failed_tests: Some(-1),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTestsRequest {
    /// List of test classes and methods to run.
    pub tests: Vec<TestItem>,
    /// Maximum number of test failures before aborting (`-1` for unlimited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_failed_tests: Option<i32>,
}

/// A test class and optional specific methods to run.
///
/// When `test_methods` is `None`, all test methods in the class are executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestItem {
    /// The ID of the ApexClass to test (key prefix `01p`).
    pub class_id: String,
    /// Specific test methods to run. If `None`, all test methods in the class
    /// are executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_methods: Option<Vec<String>>,
}

/// Result of running Apex tests synchronously.
///
/// Contains aggregate counts, individual test outcomes, and code coverage data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTestsResult {
    /// Number of tests executed.
    pub num_tests_run: i32,
    /// Number of test failures.
    pub num_failures: i32,
    /// Total execution time in milliseconds.
    pub total_time: f64,
    /// List of successful test executions.
    pub successes: Vec<TestSuccess>,
    /// List of failed test executions.
    pub failures: Vec<TestFailure>,
    /// Code coverage results.
    pub code_coverage: Vec<CodeCoverageResult>,
}

impl RunTestsResult {
    /// Returns `true` if all tests passed (zero failures).
    pub fn all_passed(&self) -> bool {
        self.num_failures == 0
    }
}

/// A successful test execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSuccess {
    /// Test result record ID (key prefix `07M`).
    pub id: String,
    /// Name of the test method.
    pub method_name: String,
    /// Name of the test class.
    pub name: String,
    /// Execution time in milliseconds.
    pub time: f64,
}

/// A failed test execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestFailure {
    /// Test result record ID (key prefix `07M`).
    pub id: String,
    /// Name of the test method.
    pub method_name: String,
    /// Name of the test class.
    pub name: String,
    /// Failure message.
    pub message: String,
    /// Stack trace of the failure.
    pub stack_trace: String,
    /// Execution time in milliseconds.
    pub time: f64,
}

/// Code coverage result for an Apex class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeCoverageResult {
    /// ID of the covered class (key prefix `01p`).
    pub id: String,
    /// Name of the covered class.
    pub name: String,
    /// Total number of coverable lines.
    pub num_locations: i32,
    /// Number of lines not covered.
    pub num_locations_not_covered: i32,
    /// Type of the covered entity (e.g., `"ApexClass"`).
    #[serde(rename = "type")]
    pub type_: String,
}

// ── ToolingHandler impl ──────────────────────────────────────────────

impl<A: crate::auth::Authenticator> super::ToolingHandler<A> {
    /// Runs Apex tests synchronously.
    ///
    /// Blocks until all tests complete and returns detailed results
    /// including pass/fail status and code coverage.
    ///
    /// # Arguments
    ///
    /// * `request` - The test run request specifying classes and methods.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::tooling::run_tests::{RunTestsRequest, TestItem};
    ///
    /// let request = RunTestsRequest {
    ///     tests: vec![TestItem {
    ///         class_id: "01pxx000000ABCDE".to_string(),
    ///         test_methods: None,
    ///     }],
    ///     max_failed_tests: Some(-1),
    /// };
    ///
    /// let result = client.tooling().run_tests(&request).await?;
    /// if result.all_passed() {
    ///     println!("All {} tests passed!", result.num_tests_run);
    /// }
    /// ```
    pub async fn run_tests(
        &self,
        request: &RunTestsRequest,
    ) -> crate::error::Result<RunTestsResult> {
        use crate::api::rest_operation::RestOperation;

        let url = self
            .session()
            .resolve_url("tooling/runTestsSynchronous")
            .await?;
        let req = self
            .session()
            .post(&url)
            .json(request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.session()
            .send_request_and_decode(req, "Run tests synchronous failed")
            .await
    }

    /// Runs Apex tests asynchronously.
    ///
    /// Returns immediately with a test run ID that can be used to poll for
    /// results via SOQL on `ApexTestRunResult`.
    ///
    /// # Arguments
    ///
    /// * `request` - The test run request specifying classes and methods.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use force::api::tooling::run_tests::{RunTestsRequest, TestItem};
    ///
    /// let request = RunTestsRequest {
    ///     tests: vec![TestItem {
    ///         class_id: "01pxx000000ABCDE".to_string(),
    ///         test_methods: Some(vec!["testMethod1".to_string()]),
    ///     }],
    ///     max_failed_tests: None,
    /// };
    ///
    /// let test_run_id = client.tooling().run_tests_async(&request).await?;
    /// println!("Test run ID: {}", test_run_id);
    /// ```
    pub async fn run_tests_async(&self, request: &RunTestsRequest) -> crate::error::Result<String> {
        use crate::api::rest_operation::RestOperation;

        let url = self
            .session()
            .resolve_url("tooling/runTestsAsynchronous")
            .await?;
        let req = self
            .session()
            .post(&url)
            .json(request)
            .build()
            .map_err(crate::error::HttpError::from)?;
        self.session()
            .send_request_and_decode(req, "Run tests asynchronous failed")
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::builder;
    use crate::test_support::{MockAuthenticator, Must};
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ── Serialization tests ──────────────────────────────────────────

    #[test]
    fn test_run_tests_request_serialization() {
        let request = RunTestsRequest {
            tests: vec![TestItem {
                class_id: "01pxx000000ABCDE".to_string(),
                test_methods: Some(vec!["testMethod1".to_string(), "testMethod2".to_string()]),
            }],
            max_failed_tests: Some(-1),
        };

        let json = serde_json::to_value(&request).must();

        assert_eq!(json["tests"][0]["classId"], "01pxx000000ABCDE");
        assert_eq!(json["tests"][0]["testMethods"][0], "testMethod1");
        assert_eq!(json["tests"][0]["testMethods"][1], "testMethod2");
        assert_eq!(json["maxFailedTests"], -1);
    }

    #[test]
    fn test_run_tests_request_serialization_skips_none_fields() {
        let request = RunTestsRequest {
            tests: vec![TestItem {
                class_id: "01pxx000000ABCDE".to_string(),
                test_methods: None,
            }],
            max_failed_tests: None,
        };

        let json = serde_json::to_value(&request).must();

        // Optional fields with None should be absent
        assert!(json["tests"][0].get("testMethods").is_none());
        assert!(json.get("maxFailedTests").is_none());
    }

    // ── Synchronous run_tests tests ──────────────────────────────────

    #[tokio::test]
    async fn test_run_tests_all_pass() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let request_body = serde_json::json!({
            "tests": [{
                "classId": "01pxx000000ABCDE",
                "testMethods": ["testMethod1", "testMethod2"]
            }],
            "maxFailedTests": -1
        });

        let response_body = serde_json::json!({
            "numTestsRun": 2,
            "numFailures": 0,
            "totalTime": 1234.0,
            "successes": [
                {
                    "id": "07Mxx000000001A",
                    "methodName": "testMethod1",
                    "name": "MyTestClass",
                    "time": 500.0
                },
                {
                    "id": "07Mxx000000002B",
                    "methodName": "testMethod2",
                    "name": "MyTestClass",
                    "time": 734.0
                }
            ],
            "failures": [],
            "codeCoverage": []
        });

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/tooling/runTestsSynchronous"))
            .and(body_json(&request_body))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let request = RunTestsRequest {
            tests: vec![TestItem {
                class_id: "01pxx000000ABCDE".to_string(),
                test_methods: Some(vec!["testMethod1".to_string(), "testMethod2".to_string()]),
            }],
            max_failed_tests: Some(-1),
        };

        let result = client.tooling().run_tests(&request).await.must();

        assert_eq!(result.num_tests_run, 2);
        assert_eq!(result.num_failures, 0);
        assert!(result.all_passed());
        assert!((result.total_time - 1234.0).abs() < f64::EPSILON);
        assert_eq!(result.successes.len(), 2);
        assert_eq!(result.successes[0].id, "07Mxx000000001A");
        assert_eq!(result.successes[0].method_name, "testMethod1");
        assert_eq!(result.successes[0].name, "MyTestClass");
        assert!((result.successes[0].time - 500.0).abs() < f64::EPSILON);
        assert!(result.failures.is_empty());
    }

    #[tokio::test]
    async fn test_run_tests_with_failures() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let response_body = serde_json::json!({
            "numTestsRun": 2,
            "numFailures": 1,
            "totalTime": 800.0,
            "successes": [
                {
                    "id": "07Mxx000000001A",
                    "methodName": "testInsert",
                    "name": "AccountTest",
                    "time": 300.0
                }
            ],
            "failures": [
                {
                    "id": "07Mxx000000002B",
                    "methodName": "testDelete",
                    "name": "AccountTest",
                    "message": "System.AssertException: Assertion Failed",
                    "stackTrace": "Class.AccountTest.testDelete: line 42, column 1",
                    "time": 500.0
                }
            ],
            "codeCoverage": []
        });

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/tooling/runTestsSynchronous"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let request = RunTestsRequest {
            tests: vec![TestItem {
                class_id: "01pxx000000FGHIJ".to_string(),
                test_methods: None,
            }],
            max_failed_tests: None,
        };

        let result = client.tooling().run_tests(&request).await.must();

        assert_eq!(result.num_tests_run, 2);
        assert_eq!(result.num_failures, 1);
        assert!(!result.all_passed());
        assert_eq!(result.successes.len(), 1);
        assert_eq!(result.failures.len(), 1);

        let failure = &result.failures[0];
        assert_eq!(failure.id, "07Mxx000000002B");
        assert_eq!(failure.method_name, "testDelete");
        assert_eq!(failure.name, "AccountTest");
        assert_eq!(failure.message, "System.AssertException: Assertion Failed");
        assert_eq!(
            failure.stack_trace,
            "Class.AccountTest.testDelete: line 42, column 1"
        );
        assert!((failure.time - 500.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_run_tests_code_coverage() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        let response_body = serde_json::json!({
            "numTestsRun": 1,
            "numFailures": 0,
            "totalTime": 600.0,
            "successes": [
                {
                    "id": "07Mxx000000001A",
                    "methodName": "testInsert",
                    "name": "AccountTest",
                    "time": 600.0
                }
            ],
            "failures": [],
            "codeCoverage": [
                {
                    "id": "01pxx000000ABCDE",
                    "name": "AccountService",
                    "numLocations": 50,
                    "numLocationsNotCovered": 5,
                    "type": "ApexClass"
                },
                {
                    "id": "01pxx000000FGHIJ",
                    "name": "AccountTriggerHandler",
                    "numLocations": 20,
                    "numLocationsNotCovered": 0,
                    "type": "ApexClass"
                }
            ]
        });

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/tooling/runTestsSynchronous"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let request = RunTestsRequest {
            tests: vec![TestItem {
                class_id: "01pxx000000KLMNO".to_string(),
                test_methods: Some(vec!["testInsert".to_string()]),
            }],
            max_failed_tests: Some(-1),
        };

        let result = client.tooling().run_tests(&request).await.must();

        assert!(result.all_passed());
        assert_eq!(result.code_coverage.len(), 2);

        let cov = &result.code_coverage[0];
        assert_eq!(cov.id, "01pxx000000ABCDE");
        assert_eq!(cov.name, "AccountService");
        assert_eq!(cov.num_locations, 50);
        assert_eq!(cov.num_locations_not_covered, 5);
        assert_eq!(cov.type_, "ApexClass");

        let cov2 = &result.code_coverage[1];
        assert_eq!(cov2.name, "AccountTriggerHandler");
        assert_eq!(cov2.num_locations, 20);
        assert_eq!(cov2.num_locations_not_covered, 0);
    }

    // ── Asynchronous run_tests_async tests ───────────────────────────

    #[tokio::test]
    async fn test_run_tests_async_returns_id() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/tooling/runTestsAsynchronous"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!("707xx0000000001")),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let request = RunTestsRequest {
            tests: vec![TestItem {
                class_id: "01pxx000000ABCDE".to_string(),
                test_methods: Some(vec!["testMethod1".to_string()]),
            }],
            max_failed_tests: None,
        };

        let test_run_id = client.tooling().run_tests_async(&request).await.must();

        assert_eq!(test_run_id, "707xx0000000001");
    }

    // ── Error handling tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_run_tests_http_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/tooling/runTestsSynchronous"))
            .respond_with(
                ResponseTemplate::new(500).set_body_json(serde_json::json!([{
                    "message": "Internal server error",
                    "errorCode": "INTERNAL_SERVER_ERROR"
                }])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let request = RunTestsRequest {
            tests: vec![TestItem {
                class_id: "01pxx000000ABCDE".to_string(),
                test_methods: None,
            }],
            max_failed_tests: None,
        };

        let result = client.tooling().run_tests(&request).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_run_tests_async_http_error() {
        let mock_server = MockServer::start().await;
        let auth = MockAuthenticator::new("test_token", &mock_server.uri());
        let client = builder().authenticate(auth).build().await.must();

        Mock::given(method("POST"))
            .and(path("/services/data/v60.0/tooling/runTestsAsynchronous"))
            .respond_with(
                ResponseTemplate::new(500).set_body_json(serde_json::json!([{
                    "message": "Service unavailable",
                    "errorCode": "SERVER_ERROR"
                }])),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let request = RunTestsRequest {
            tests: vec![TestItem {
                class_id: "01pxx000000ABCDE".to_string(),
                test_methods: None,
            }],
            max_failed_tests: None,
        };

        let result = client.tooling().run_tests_async(&request).await;
        let Err(err) = result else {
            panic!("Expected an error");
        };
        assert!(
            matches!(
                err,
                crate::error::ForceError::Api(_) | crate::error::ForceError::Http(_)
            ),
            "Expected Api or Http error, got: {err}"
        );
    }

    // ── all_passed helper tests ──────────────────────────────────────

    #[test]
    fn test_all_passed_with_zero_failures() {
        let result = RunTestsResult {
            num_tests_run: 5,
            num_failures: 0,
            total_time: 2000.0,
            successes: vec![],
            failures: vec![],
            code_coverage: vec![],
        };
        assert!(result.all_passed());
    }

    #[test]
    fn test_all_passed_with_failures() {
        let result = RunTestsResult {
            num_tests_run: 5,
            num_failures: 2,
            total_time: 2000.0,
            successes: vec![],
            failures: vec![],
            code_coverage: vec![],
        };
        assert!(!result.all_passed());
    }
}
