use super::*;

#[test]
fn a_column_is_one_unit_and_a_row_is_one_unit_tall() {
    let text = TextMetrics::new("alpha beta gamma");
    let mut measure = CellMeasure;
    assert_eq!(measure.line_height(), 1.0);
    assert_eq!(measure.intrinsic(&text), (16.0, 5.0));
}

#[test]
fn a_fractional_width_floors_to_whole_columns() {
    let text = TextMetrics::new("abcdefghij");
    let mut measure = CellMeasure;
    // 9.9 units is nine columns, so the word breaks; 10.0 is ten and it
    // does not.
    assert_eq!(measure.wrap(&text, 9.9).len(), 2);
    assert_eq!(measure.wrap(&text, 10.0).len(), 1);
}

#[test]
fn the_height_is_the_rows_its_own_wrap_produced() {
    let text = TextMetrics::new("one two three four five six seven");
    let mut measure = CellMeasure;
    for width in [4.0, 7.0, 12.0, 40.0] {
        let rows = measure.wrap(&text, width).len();
        #[allow(clippy::cast_precision_loss)]
        let rows = rows as f32;
        assert_eq!(measure.height(&text, width), rows);
    }
}
