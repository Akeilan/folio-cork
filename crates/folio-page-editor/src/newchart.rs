//! Making a chart: the chart part a document carries, and the little
//! workbook that holds its numbers, as Word writes them.

use crate::eng::package::Package;

/// The numbers a chart shows: a row of categories down the side, a column of
/// series across the top, and a value where they meet.
#[derive(Clone, Debug)]
pub struct ChartData {
    pub rows: Vec<String>,
    pub cols: Vec<String>,
    pub values: Vec<Vec<f64>>,
}

impl Default for ChartData {
    fn default() -> Self {
        ChartData {
            rows: ["Category 1", "Category 2", "Category 3", "Category 4"].iter().map(|s| s.to_string()).collect(),
            cols: ["Series 1", "Series 2"].iter().map(|s| s.to_string()).collect(),
            values: vec![vec![4.3, 2.4], vec![2.5, 4.4], vec![3.5, 1.8], vec![4.5, 2.8]],
        }
    }
}

impl ChartData {
    /// Numbers written out as lines: the first line names the series, each
    /// line after it is a category and its values.
    pub fn parse(text: &str) -> Option<ChartData> {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let head = lines.next()?;
        let cols: Vec<String> = cells(head).into_iter().skip(1).collect();
        if cols.is_empty() {
            return None;
        }
        let mut rows = Vec::new();
        let mut values = Vec::new();
        for line in lines {
            let c = cells(line);
            if c.is_empty() {
                continue;
            }
            rows.push(c[0].clone());
            let mut v: Vec<f64> = c[1..].iter().map(|x| x.trim().parse::<f64>().unwrap_or(0.0)).collect();
            v.resize(cols.len(), 0.0);
            values.push(v);
        }
        if rows.is_empty() {
            return None;
        }
        Some(ChartData { rows, cols, values })
    }

    pub fn as_text(&self) -> String {
        let mut out = String::new();
        out.push(',');
        out.push_str(&self.cols.join(","));
        out.push('\n');
        for (i, r) in self.rows.iter().enumerate() {
            out.push_str(r);
            for v in self.values.get(i).map(|v| v.as_slice()).unwrap_or(&[]) {
                out.push(',');
                out.push_str(&trim_num(*v));
            }
            out.push('\n');
        }
        out
    }
}

fn cells(line: &str) -> Vec<String> {
    let sep = if line.contains('\t') { '\t' } else { ',' };
    line.split(sep).map(|c| c.trim().to_string()).collect()
}

fn trim_num(v: f64) -> String {
    let s = format!("{v}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Excel's own column letters: A, B ... Z, AA.
fn col_letter(mut i: usize) -> String {
    let mut out = String::new();
    i += 1;
    while i > 0 {
        let r = (i - 1) % 26;
        out.insert(0, (b'A' + r as u8) as char);
        i = (i - 1) / 26;
    }
    out
}

/// The colours Word gives the first series of a new chart.
const SERIES_COLORS: [&str; 6] = ["4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47"];

/// A chart part for the numbers given. `kind` is "bar", "column", "line" or
/// "pie".
pub fn chart_xml(kind: &str, data: &ChartData, title: &str) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n");
    s.push_str("<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
                xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
                xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">");
    s.push_str("<c:chart>");
    if !title.trim().is_empty() {
        s.push_str(&format!(
            "<c:title><c:tx><c:rich><a:bodyPr/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx>\
             <c:overlay val=\"0\"/></c:title><c:autoTitleDeleted val=\"0\"/>",
            esc(title.trim())
        ));
    } else {
        s.push_str("<c:autoTitleDeleted val=\"1\"/>");
    }
    s.push_str("<c:plotArea><c:layout/>");
    let el = match kind {
        "line" => "c:lineChart",
        "pie" => "c:pieChart",
        _ => "c:barChart",
    };
    s.push_str(&format!("<{el}>"));
    if el == "c:barChart" {
        s.push_str(if kind == "bar" { "<c:barDir val=\"bar\"/>" } else { "<c:barDir val=\"col\"/>" });
        s.push_str("<c:grouping val=\"clustered\"/>");
    } else if el == "c:lineChart" {
        s.push_str("<c:grouping val=\"standard\"/>");
    }
    s.push_str("<c:varyColors val=\"0\"/>");
    let series = if kind == "pie" { 1 } else { data.cols.len() };
    for (i, name) in data.cols.iter().take(series.max(1)).enumerate() {
        s.push_str(&format!("<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>"));
        let cell = format!("Sheet1!${}$1", col_letter(i + 1));
        s.push_str(&format!(
            "<c:tx><c:strRef><c:f>{cell}</c:f><c:strCache><c:ptCount val=\"1\"/>\
             <c:pt idx=\"0\"><c:v>{}</c:v></c:pt></c:strCache></c:strRef></c:tx>",
            esc(name)
        ));
        let colour = SERIES_COLORS[i % SERIES_COLORS.len()];
        if kind == "line" {
            s.push_str(&format!(
                "<c:spPr><a:ln w=\"28575\" cap=\"rnd\"><a:solidFill><a:srgbClr val=\"{colour}\"/></a:solidFill>\
                 <a:round/></a:ln></c:spPr><c:marker><c:symbol val=\"none\"/></c:marker>"
            ));
        } else {
            s.push_str(&format!("<c:spPr><a:solidFill><a:srgbClr val=\"{colour}\"/></a:solidFill></c:spPr>"));
        }
        // the categories
        s.push_str("<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$");
        s.push_str(&format!("{}</c:f><c:strCache><c:ptCount val=\"{}\"/>", data.rows.len() + 1, data.rows.len()));
        for (j, r) in data.rows.iter().enumerate() {
            s.push_str(&format!("<c:pt idx=\"{j}\"><c:v>{}</c:v></c:pt>", esc(r)));
        }
        s.push_str("</c:strCache></c:strRef></c:cat>");
        // and the values
        let letter = col_letter(i + 1);
        s.push_str(&format!(
            "<c:val><c:numRef><c:f>Sheet1!${letter}$2:${letter}${}</c:f><c:numCache>\
             <c:formatCode>General</c:formatCode><c:ptCount val=\"{}\"/>",
            data.rows.len() + 1,
            data.rows.len()
        ));
        for (j, row) in data.values.iter().enumerate() {
            let v = row.get(i).copied().unwrap_or(0.0);
            s.push_str(&format!("<c:pt idx=\"{j}\"><c:v>{}</c:v></c:pt>", trim_num(v)));
        }
        s.push_str("</c:numCache></c:numRef></c:val></c:ser>");
    }
    if el == "c:barChart" {
        s.push_str("<c:gapWidth val=\"182\"/><c:overlap val=\"-27\"/>");
    }
    if el != "c:pieChart" {
        s.push_str("<c:axId val=\"111111111\"/><c:axId val=\"222222222\"/>");
    }
    s.push_str(&format!("</{el}>"));
    if el != "c:pieChart" {
        s.push_str(
            "<c:catAx><c:axId val=\"111111111\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>\
             <c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"222222222\"/></c:catAx>\
             <c:valAx><c:axId val=\"222222222\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>\
             <c:delete val=\"0\"/><c:axPos val=\"l\"/><c:majorGridlines/><c:crossAx val=\"111111111\"/></c:valAx>",
        );
    }
    s.push_str("</c:plotArea>");
    s.push_str("<c:legend><c:legendPos val=\"b\"/><c:overlay val=\"0\"/></c:legend>");
    s.push_str("<c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/></c:chart></c:chartSpace>");
    s
}

/// The workbook a chart keeps its numbers in: Sheet1, categories down A and
/// the series across the top.
pub fn chart_workbook(data: &ChartData) -> Vec<u8> {
    let mut strings: Vec<String> = Vec::new();
    let sidx = |s: &str, strings: &mut Vec<String>| -> usize {
        if let Some(i) = strings.iter().position(|x| x == s) {
            return i;
        }
        strings.push(s.to_string());
        strings.len() - 1
    };
    let mut sheet = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n\
         <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    // the first row names the series
    sheet.push_str("<row r=\"1\">");
    for (i, c) in data.cols.iter().enumerate() {
        let n = sidx(c, &mut strings);
        sheet.push_str(&format!("<c r=\"{}1\" t=\"s\"><v>{n}</v></c>", col_letter(i + 1)));
    }
    sheet.push_str("</row>");
    for (j, r) in data.rows.iter().enumerate() {
        let rn = j + 2;
        sheet.push_str(&format!("<row r=\"{rn}\">"));
        let n = sidx(r, &mut strings);
        sheet.push_str(&format!("<c r=\"A{rn}\" t=\"s\"><v>{n}</v></c>"));
        for (i, v) in data.values.get(j).map(|v| v.as_slice()).unwrap_or(&[]).iter().enumerate() {
            sheet.push_str(&format!("<c r=\"{}{rn}\"><v>{}</v></c>", col_letter(i + 1), trim_num(*v)));
        }
        sheet.push_str("</row>");
    }
    sheet.push_str("</sheetData></worksheet>");

    let mut shared = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n\
         <sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{}\" uniqueCount=\"{}\">",
        strings.len(),
        strings.len()
    );
    for s in &strings {
        shared.push_str(&format!("<si><t>{}</t></si>", esc(s)));
    }
    shared.push_str("</sst>");

    let types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n\
        <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
        <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
        <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
        <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
        <Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>\
        <Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>\
        </Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
        </Relationships>";
    let wb = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n\
        <workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
        xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
        <sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>";
    let wb_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>\
        <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/>\
        </Relationships>";

    let parts: Vec<(&str, Vec<u8>)> = vec![
        ("[Content_Types].xml", types.as_bytes().to_vec()),
        ("_rels/.rels", rels.as_bytes().to_vec()),
        ("xl/workbook.xml", wb.as_bytes().to_vec()),
        ("xl/_rels/workbook.xml.rels", wb_rels.as_bytes().to_vec()),
        ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
        ("xl/sharedStrings.xml", shared.into_bytes()),
    ];
    let pkg = Package::from_parts(parts.into_iter().map(|(n, b)| (n.to_string(), b)).collect());
    pkg.write().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_lettered_as_excel_letters_them() {
        assert_eq!(col_letter(0), "A");
        assert_eq!(col_letter(25), "Z");
        assert_eq!(col_letter(26), "AA");
    }

    #[test]
    fn numbers_written_out_come_back() {
        let d = ChartData::default();
        let back = ChartData::parse(&d.as_text()).expect("read back");
        assert_eq!(back.rows, d.rows);
        assert_eq!(back.cols, d.cols);
        assert_eq!(back.values, d.values);
    }

    #[test]
    fn the_chart_part_holds_the_numbers() {
        let d = ChartData::default();
        let xml = chart_xml("column", &d, "A title");
        assert!(xml.contains("c:barChart"), "a column chart is a bar chart turned up");
        assert!(xml.contains("<c:v>Series 1</c:v>"));
        assert!(xml.contains("<c:v>4.3</c:v>"));
        assert!(xml.contains("A title"));
        let book = chart_workbook(&d);
        assert!(book.starts_with(b"PK"), "the workbook is a zip");
        assert!(book.len() > 500);
    }
}
