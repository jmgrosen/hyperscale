from asyncio import Event

import lvgl as lv

from observable import Observable

ICON_SIZE = 60

def make_cb(name, num):
    s = f"{name} {num}"
    def cb(e):
        print(s)
    return cb

class IconMenuView:
    def __init__(self, selected_item: Observable, items, parent=None):
        self.group = lv.group_create()
        self.group.set_wrap(False)
        self.root = lv.obj(parent)
        self.root.set_size(ICON_SIZE*2, ICON_SIZE*4)
        # self.root.clear_flag(lv.obj.FLAG.SCROLLABLE)
        # self.root.add_flag(lv.obj.FLAG.CLICKABLE)
        # self.root.add_flag(lv.obj.FLAG.SCROLL_ON_FOCUS)
        self.root.set_flex_flow(lv.FLEX_FLOW.COLUMN)
        self.root.set_scroll_dir(lv.DIR.VER)
        self.root.set_scroll_snap_y(lv.SCROLL_SNAP.CENTER)
        self.root.set_style_pad_all(0, 0)
        # self.root.set_scrollbar_mode(lv.SCROLLBAR_MODE.OFF)
        # self.root.add_event(make_cb("scroll", ""), lv.EVENT.SCROLL, None)

        self.done = Event()

        icon_focused_style = lv.style_t()
        icon_focused_style.set_outline_width(2)
        icon_focused_style.set_outline_color(lv.color_white())

        self.buttons = [lv.btn(self.root) for _ in items]
        for i, (item, button) in enumerate(zip(items, self.buttons)):
            self.group.add_obj(button)
            button.set_size(ICON_SIZE, ICON_SIZE)
            button.add_flag(lv.obj.FLAG.SCROLL_ON_FOCUS | lv.obj.FLAG.EVENT_BUBBLE)
            button.add_event(selected_item.update_to(i), lv.EVENT.FOCUSED, None)
            # button.add_event(make_cb("defocus", i), lv.EVENT.DEFOCUSED, None)
            button.add_event(lambda _: self.done.set(), lv.EVENT.CLICKED, None)
            button.add_event(self.handle_esc, lv.EVENT.KEY, None)
            icon = lv.img(button)
            icon.center()
            icon.set_src(item)
            # icon.clear_flag(lv.obj.FLAG.CLICKABLE)
            # icon.clear_flag(lv.obj.FLAG.CHECKABLE)
            # icon.clear_flag(lv.obj.FLAG.CLICK_FOCUSABLE)
            # icon.set_size(ICON_SIZE, ICON_SIZE)
            # icon.add_style(icon_focused_style, lv.STATE.FOCUSED)

        self.root.update_snap(lv.ANIM.OFF)

    async def render(self):
        pass

    def handle_esc(self, e):
        if e.get_key() == lv.KEY.ESC:
            self.done.set()
            e.stop_bubbling = 1
