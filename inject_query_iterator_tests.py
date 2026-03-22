import sys

file_path = "crates/force/src/types/query.rs"

with open(file_path, "r") as f:
    content = f.read()

test = """
    #[test]
    fn test_query_iterator_empty_middle_page() {
        let page1: QueryResult<i32> = QueryResult::with_next_page(5, vec![1, 2], "/next1".into());
        let page2: QueryResult<i32> = QueryResult::with_next_page(5, vec![], "/next2".into());
        let page3: QueryResult<i32> = QueryResult::new(5, true, vec![4, 5]);

        let iter = QueryIterator::new(vec![page1, page2, page3]);
        let collected: Vec<i32> = iter.collect();

        assert_eq!(collected, vec![1, 2, 4, 5]);
    }
}
"""

content = content.replace("}\n", "}\n" + test)

# We actually only want to replace the last closing brace
with open(file_path, "r") as f:
    lines = f.readlines()

new_lines = lines[:-1]
new_lines.append(test)

with open(file_path, "w") as f:
    f.writelines(new_lines)

print("Tests injected into query.rs")
