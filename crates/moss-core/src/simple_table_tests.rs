use super::*;
#[test]
fn markdown_round_trip_preserves_cells_and_header_settings() {
    let table = SimpleTable {
        version: 1,
        rows: vec![
            vec![" a|b ".into(), "slash\\and<br> &amp;".into()],
            vec!["two\nlines".into(), "😀".into()],
        ],
        header_row: false,
        header_column: true,
    };
    assert_eq!(
        SimpleTable::from_markdown(&table.to_markdown().unwrap()).unwrap(),
        table
    );
    let markdown = table.to_markdown().unwrap();
    assert!(markdown.starts_with("<!-- moss:header-row="));
    assert!(SimpleTable::from_markdown("| A | B |\n| --- | --- |\n| x |").is_err());
}
#[test]
fn dimensions_and_invalid_data_are_guarded() {
    let mut table = SimpleTable::default();
    table.insert_row(0).unwrap();
    table.insert_column(1).unwrap();
    assert_eq!((table.rows.len(), table.rows[0].len()), (4, 4));
    table.delete_row(0).unwrap();
    table.delete_column(0).unwrap();
    table.validate().unwrap();
    table.rows[0].push("bad".into());
    assert!(table.validate().is_err());
}
