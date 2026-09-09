"""Independent XLSX inspector for Rust/browser tests; Python 3 standard library only."""

import json
import posixpath
import sys
import xml.etree.ElementTree as ET
import zipfile


def read_workbook(filename):
    ns = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
    with zipfile.ZipFile(filename) as archive:
        assert archive.testzip() is None, "Corrupt XLSX archive"
        # Parsing all XML parts also catches malformed output outside worksheet cells.
        for name in archive.namelist():
            if name.endswith((".xml", ".rels")):
                ET.fromstring(archive.read(name))
        strings = []
        if "xl/sharedStrings.xml" in archive.namelist():
            strings = ["".join(item.itertext()) for item in ET.fromstring(archive.read("xl/sharedStrings.xml"))]
        styles = ET.fromstring(archive.read("xl/styles.xml"))
        formats = {0: "General", 2: "0.00", 14: "mm-dd-yy"}
        for item in styles.findall("s:numFmts/s:numFmt", ns):
            formats[int(item.attrib["numFmtId"])] = item.attrib["formatCode"]
        fonts = list(styles.find("s:fonts", ns))
        cell_styles = list(styles.find("s:cellXfs", ns))
        rels = {item.attrib["Id"]: item.attrib["Target"] for item in ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))}
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheets = []
        for sheet in workbook.findall("s:sheets/s:sheet", ns):
            rel = sheet.attrib["{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"]
            target = rels[rel]
            target = target.lstrip("/") if target.startswith("/") else posixpath.normpath("xl/" + target)
            root = ET.fromstring(archive.read(target))
            rows = []
            for row in root.findall("s:sheetData/s:row", ns):
                cells = {}
                for cell in row:
                    style = cell_styles[int(cell.attrib.get("s", "0"))]
                    kind = cell.attrib.get("t", "n")
                    raw = cell.findtext("s:v", default="", namespaces=ns)
                    if kind == "s":
                        value = strings[int(raw)]
                    elif kind == "inlineStr":
                        value = "".join(cell.find("s:is", ns).itertext())
                    elif kind == "n" and raw:
                        value = float(raw)
                    else:
                        value = raw
                    cells[cell.attrib["r"]] = {
                        "value": value, "type": kind,
                        "format": formats.get(int(style.attrib["numFmtId"]), "unknown"),
                        "bold": fonts[int(style.attrib["fontId"])].find("s:b", ns) is not None,
                        "formula": cell.find("s:f", ns) is not None,
                    }
                rows.append(cells)
            pane = root.find("s:sheetViews/s:sheetView/s:pane", ns)
            auto_filter = root.find("s:autoFilter", ns)
            sheets.append({
                "name": sheet.attrib["name"], "rows": rows,
                "pane": pane.attrib if pane is not None else {},
                "filter": auto_filter.attrib if auto_filter is not None else {},
                "columns": [item.attrib for item in root.findall("s:cols/s:col", ns)],
            })
        return sheets


if __name__ == "__main__":
    print(json.dumps(read_workbook(sys.argv[1]), ensure_ascii=False))
