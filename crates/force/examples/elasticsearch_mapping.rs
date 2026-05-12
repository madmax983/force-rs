//! Example showing how to generate an Elasticsearch mapping from a Salesforce `SObject` describe metadata.

use force::schema::generate_elasticsearch_mapping;
use force::types::SObjectDescribe;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let describe_json = r#"{
        "activateable": false, "createable": true, "custom": false, "customSetting": false,
        "deletable": true, "deprecatedAndHidden": false, "feedEnabled": true,
        "hasSubtypes": false, "isSubtype": false, "label": "Account", "labelPlural": "Accounts",
        "layoutable": true, "mergeable": true, "mruEnabled": true, "name": "Account",
        "queryable": true, "replicateable": true, "retrieveable": true, "searchable": true,
        "triggerable": true, "undeletable": true, "updateable": true, "urls": {},
        "childRelationships": [], "recordTypeInfos": [], "fields": [
            {
                "aggregatable": true, "autoNumber": false, "byteLength": 18, "calculated": false,
                "cascadeDelete": false, "caseSensitive": false, "createable": false, "custom": false,
                "defaultedOnCreate": true, "dependentPicklist": false, "deprecatedAndHidden": false,
                "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                "idLookup": true, "label": "Account ID", "length": 18, "name": "Id", "nameField": false,
                "namePointing": false, "nillable": false, "permissionable": false,
                "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
                "soapType": "xsd:id", "sortable": true, "type": "id", "unique": true, "updateable": false,
                "writeRequiresMasterRead": false
            },
            {
                "aggregatable": true, "autoNumber": false, "byteLength": 255, "calculated": false,
                "cascadeDelete": false, "caseSensitive": false, "createable": true, "custom": false,
                "defaultedOnCreate": false, "dependentPicklist": false, "deprecatedAndHidden": false,
                "digits": 0, "displayLocationInDecimal": false, "encrypted": false, "externalId": false,
                "filterable": true, "groupable": true, "highScaleNumber": false, "htmlFormatted": false,
                "idLookup": false, "label": "Account Name", "length": 255, "name": "Name", "nameField": true,
                "namePointing": false, "nillable": false, "permissionable": false,
                "polymorphicForeignKey": false, "precision": 0, "queryByDistance": false,
                "referenceTo": [], "restrictedDelete": false, "restrictedPicklist": false, "scale": 0,
                "soapType": "xsd:string", "sortable": true, "type": "string", "unique": false, "updateable": true,
                "writeRequiresMasterRead": false
            }
        ]
    }"#;
    let describe: SObjectDescribe = serde_json::from_str(describe_json)?;
    let mapping = generate_elasticsearch_mapping(&describe);
    println!("{}", serde_json::to_string_pretty(&mapping)?);
    Ok(())
}
