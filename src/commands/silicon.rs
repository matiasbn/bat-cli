use std::fs;

pub fn create_figure(
    source_path: &str,
    dest_folder_path: &str,
    file_name: &str,
    starting_line: usize,
    ending_line: usize,
) {
    let content = fs::read_to_string(source_path)
        .unwrap()
        .lines()
        .collect::<Vec<_>>()[starting_line..=ending_line]
        .to_vec()
        .join("\n");
    // write the temporary markdown file
    let (dest_md_path, dest_png_path) = get_dest_md_and_png_path(file_name, dest_folder_path);
    fs::write(&dest_md_path, content.as_str()).unwrap();
    // take the snapshot
    take_silicon_snapshot(&dest_md_path, &dest_png_path, starting_line);
    fs::remove_file(dest_md_path).unwrap();
}

fn get_dest_md_and_png_path(file_name: &str, dest_folder_path: &str) -> (String, String) {
    (
        format!("{dest_folder_path}/{file_name}.md"),
        format!("{dest_folder_path}/{file_name}.png"),
    )
}

fn take_silicon_snapshot(source_md_path: &str, dest_png_path: &str, offset: usize) {
    let offset = format!("{offset}");
    let args = vec![
        "--no-window-controls",
        "--language",
        "Rust",
        "--line-offset",
        offset.as_str(),
        "--theme",
        "Visual Studio Dark+",
        "--pad-horiz",
        "40",
        "--pad-vert",
        "40",
        "--background",
        "#d3d4d5",
        "--font",
        "Hack=13",
        "--output",
        dest_png_path,
        source_md_path,
    ];
    std::process::Command::new("silicon")
        .args(args)
        .output()
        .unwrap();
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
