//! OpenAPI 3.0 specification generator for Salesforce SObject Describe metadata.
#[cfg(feature = "schema")]
use crate::api::rest::describe::{FieldType, SObjectDescribe};
use std::fmt::Write;

/// Experimental utility to generate OpenAPI 3.0 specifications from SObject describe metadata.
#[cfg(feature = "schema")]
#[derive(Debug, Default)]
pub struct OpenApiGenerator;

#[cfg(feature = "schema")]
impl OpenApiGenerator {
    /// Generates a YAML OpenAPI 3.0 specification for an SObject's REST API endpoints.
    pub fn generate_openapi(&self, describe: &SObjectDescribe) -> String {
        let mut out = String::with_capacity(describe.fields.len() * 256 + 1024);
        let object_name = &describe.name;

        // Header
        let _ = writeln!(out, "openapi: 3.0.3");
        let _ = writeln!(out, "info:");
        let _ = writeln!(out, "  title: {} API", object_name);
        let _ = writeln!(out, "  version: 1.0.0");
        if !describe.label.is_empty() {
            let _ = writeln!(out, "  description: REST API for {}", describe.label);
        }

        // Paths
        let _ = writeln!(out, "paths:");

        // Collection Path (/sobjects/{SObject}/)
        let _ = writeln!(out, "  /services/data/vXX.X/sobjects/{}/:", object_name);
        // POST (Create)
        let _ = writeln!(out, "    post:");
        let _ = writeln!(out, "      summary: Create {}", object_name);
        let _ = writeln!(out, "      requestBody:");
        let _ = writeln!(out, "        required: true");
        let _ = writeln!(out, "        content:");
        let _ = writeln!(out, "          application/json:");
        let _ = writeln!(out, "            schema:");
        let _ = writeln!(
            out,
            "              $ref: '#/components/schemas/{}'",
            object_name
        );
        let _ = writeln!(out, "      responses:");
        let _ = writeln!(out, "        '201':");
        let _ = writeln!(out, "          description: Created");

        // Single Record Path (/sobjects/{SObject}/{id})
        let _ = writeln!(
            out,
            "  /services/data/vXX.X/sobjects/{}/{{id}}:",
            object_name
        );
        let _ = writeln!(out, "    parameters:");
        let _ = writeln!(out, "      - name: id");
        let _ = writeln!(out, "        in: path");
        let _ = writeln!(out, "        required: true");
        let _ = writeln!(out, "        schema:");
        let _ = writeln!(out, "          type: string");

        // GET (Retrieve)
        let _ = writeln!(out, "    get:");
        let _ = writeln!(out, "      summary: Get {} by ID", object_name);
        let _ = writeln!(out, "      responses:");
        let _ = writeln!(out, "        '200':");
        let _ = writeln!(out, "          description: Success");
        let _ = writeln!(out, "          content:");
        let _ = writeln!(out, "            application/json:");
        let _ = writeln!(out, "              schema:");
        let _ = writeln!(
            out,
            "                $ref: '#/components/schemas/{}'",
            object_name
        );

        // PATCH (Update)
        let _ = writeln!(out, "    patch:");
        let _ = writeln!(out, "      summary: Update {}", object_name);
        let _ = writeln!(out, "      requestBody:");
        let _ = writeln!(out, "        required: true");
        let _ = writeln!(out, "        content:");
        let _ = writeln!(out, "          application/json:");
        let _ = writeln!(out, "            schema:");
        let _ = writeln!(
            out,
            "              $ref: '#/components/schemas/{}'",
            object_name
        );
        let _ = writeln!(out, "      responses:");
        let _ = writeln!(out, "        '204':");
        let _ = writeln!(out, "          description: Updated");

        // DELETE
        let _ = writeln!(out, "    delete:");
        let _ = writeln!(out, "      summary: Delete {}", object_name);
        let _ = writeln!(out, "      responses:");
        let _ = writeln!(out, "        '204':");
        let _ = writeln!(out, "          description: Deleted");

        // Components
        let _ = writeln!(out, "components:");
        let _ = writeln!(out, "  schemas:");
        let _ = writeln!(out, "    {}:", object_name);
        let _ = writeln!(out, "      type: object");
        let _ = writeln!(out, "      properties:");

        // Sort fields alphabetically, Id first
        let mut fields: Vec<&_> = describe.fields.iter().collect();
        fields.sort_by(|a, b| {
            if a.name == "Id" {
                std::cmp::Ordering::Less
            } else if b.name == "Id" {
                std::cmp::Ordering::Greater
            } else {
                a.name.cmp(&b.name)
            }
        });

        for field in fields {
            let _ = writeln!(out, "        {}:", field.name);
            let _ = writeln!(out, "          type: {}", Self::map_type(&field.type_));
            if !field.label.is_empty() {
                let _ = writeln!(out, "          description: {}", field.label);
            }
            if field.nillable {
                let _ = writeln!(out, "          nullable: true");
            }
        }

        out
    }

    fn map_type(field_type: &FieldType) -> &'static str {
        match field_type {
            FieldType::Boolean => "boolean",
            FieldType::Int => "integer",
            FieldType::Double | FieldType::Currency | FieldType::Percent => "number",
            FieldType::Date => "string\n          format: date",
            FieldType::Datetime => "string\n          format: date-time",
            FieldType::Time => "string\n          format: time",
            _ => "string",
        }
    }
}

#[cfg(all(test, feature = "schema"))]
mod tests {
    use super::*;
    use crate::test_support::Must;

    fn create_mock_describe(fields_json: &serde_json::Value) -> SObjectDescribe {
        let describe_json = serde_json::json!({
            "name": "Account",
            "label": "Account",
            "custom": false,
            "queryable": true,
            "activateable": false, "createable": true, "customSetting": false, "deletable": true,
            "deprecatedAndHidden": false, "feedEnabled": true, "hasSubtypes": false,
            "isSubtype": false, "keyPrefix": "001", "labelPlural": "Accounts", "layoutable": true,
            "mergeable": true, "mruEnabled": true, "replicateable": true, "retrieveable": true,
            "searchable": true, "triggerable": true, "undeletable": true, "updateable": true,
            "urls": {}, "childRelationships": [], "recordTypeInfos": [],
            "fields": fields_json.clone()
        });
        serde_json::from_value(describe_json).must()
    }

    fn mock_field(name: &str, field_type: &str, nillable: bool) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "type": field_type,
            "label": format!("Account {}", name),
            "custom": false,
            "nillable": nillable,
            "defaultedOnCreate": false,
            "calculated": false,
            "referenceTo": if field_type == "reference" { vec!["Account"] } else { vec![] },
            "createable": true, "autoNumber": false, "aggregatable": true, "byteLength": 18,
            "cascadeDelete": false, "caseSensitive": false,
            "dependentPicklist": false, "deprecatedAndHidden": false,
            "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
            "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
            "idLookup": true, "length": 18, "nameField": false, "namePointing": false,
            "permissionable": false, "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
            "restrictedDelete": false, "restrictedPicklist": false, "scale": 0, "soapType": "tns:ID",
            "sortable": true, "unique": false, "updateable": false, "writeRequiresMasterRead": false
        })
    }

    #[test]
    fn test_generate_openapi() {
        let describe = create_mock_describe(&serde_json::json!([
            mock_field("Id", "id", false),
            mock_field("Name", "string", false),
            mock_field("AnnualRevenue", "currency", true)
        ]));
        let generator = OpenApiGenerator;
        let openapi = generator.generate_openapi(&describe);

        assert!(openapi.contains("openapi: 3.0.3"));
        assert!(openapi.contains("title: Account API"));
        assert!(openapi.contains("  /services/data/vXX.X/sobjects/Account/:"));
        assert!(openapi.contains("  /services/data/vXX.X/sobjects/Account/{id}:"));

        assert!(openapi.contains("        Id:"));
        assert!(openapi.contains("          type: string"));
        assert!(openapi.contains("          description: Account Id"));

        assert!(openapi.contains("        Name:"));
        assert!(openapi.contains("          type: string"));
        assert!(openapi.contains("          description: Account Name"));

        assert!(openapi.contains("        AnnualRevenue:"));
        assert!(openapi.contains("          type: number"));
        assert!(openapi.contains("          description: Account AnnualRevenue"));
        assert!(openapi.contains("          nullable: true"));
    }
}
