use std::fs;

pub fn create_figure(
    content: &str,
    dest_folder_path: &str,
    file_name: &str,
    offset: usize,
    font_size: Option<usize>,
    show_line_number: bool,
) -> String {
    // write the temporary markdown file
    let (dest_md_path, dest_png_path) = get_dest_md_and_png_path(file_name, dest_folder_path);
    fs::write(&dest_md_path, content).unwrap();
    // take the snapshot
    take_silicon_snapshot(
        &dest_md_path,
        &dest_png_path,
        offset,
        font_size,
        show_line_number,
    );
    fs::remove_file(dest_md_path).unwrap();
    dest_png_path
}

fn get_dest_md_and_png_path(file_name: &str, dest_folder_path: &str) -> (String, String) {
    (
        format!("{dest_folder_path}/{file_name}.md"),
        format!("{dest_folder_path}/{file_name}.png"),
    )
}

fn take_silicon_snapshot(
    source_md_path: &str,
    dest_png_path: &str,
    offset: usize,
    font_size: Option<usize>,
    show_line_number: bool,
) {
    let offset = format!("{}", offset);
    let font = if let Some(size) = font_size {
        format!("Hack={size}")
    } else {
        format!("Hack=16")
    };

    let mut args = vec![
        "--no-window-controls",
        // show_line_number.as_str(),
        "--language",
        "Rust",
        "--line-offset",
        offset.as_str(),
        "--theme",
        "Monokai Extended",
        "--pad-horiz",
        "40",
        "--pad-vert",
        "40",
        "--background",
        "#d3d4d5",
        "--font",
        font.as_str(),
        "--output",
        dest_png_path,
        source_md_path,
    ];
    if !show_line_number {
        args.insert(1, "--no-line-number")
    }
    let mut child = std::process::Command::new("silicon")
        .args(args)
        .spawn()
        .unwrap();
    child.wait().unwrap();
}

pub fn delete_png_file(path: String) {
    fs::remove_file(path).unwrap();
}

pub fn check_silicon_installed() -> bool {
    let output = std::process::Command::new("silicon")
        .args(["--version"])
        .output();
    match output {
        Ok(_) => true,
        Err(_) => false,
    }
}
