app_path = defines["app"]
background_path = defines["background"]

format = "UDZO"
filesystem = "HFS+"
compression_level = 9

files = [(app_path, "Kitter.app")]
symlinks = {"Applications": "/Applications"}

background = background_path
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
window_rect = ((100, 100), (821, 479))
default_view = "icon-view"
show_icon_preview = False

arrange_by = None
grid_offset = (0, 0)
grid_spacing = 100
scroll_position = (0, 0)
label_pos = "bottom"
text_size = 14
icon_size = 128

icon_locations = {
    "Kitter.app": (250, 233),
    "Applications": (585, 233),
}

hide = [".background.tiff"]
