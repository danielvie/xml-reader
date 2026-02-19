mod xml_ops;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            xml_ops::open_file,
            xml_ops::read_chunk,
            xml_ops::search_node,
            xml_ops::cancel_search,
            xml_ops::get_first_child,
            xml_ops::get_last_child,
            xml_ops::resolve_xpath,
            xml_ops::resolve_xpath,
            xml_ops::find_parent,
            xml_ops::read_element_at_offset
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
