import time
import gc
import micropython
micropython.alloc_emergency_exception_buf(100)

from machine import Pin, I2C, freq
from nau7802 import NAU7802
from primitives import Encoder, WaitAny
import asyncio
import lvgl as lv
try:
    import aiorepl
except ImportError:
    aiorepl = None

import display
# TODO: fix this hack. we need to keep enough contiguous allocatable space for the framebuf

import blue
from observable import Observable
from qdec import QDec
from ui.weight import WeightValueView
from ui.iconmenu import IconMenuView
from ui.ingredients import IngredientsView
from ui.weighprogress import WeighingIngredientView
from ui.recipe import RecipeView
from recipe import Ingredient, InProgressRecipe, cookie_recipe
from lv_utils import fs_driver

gc.collect()

ONE_KG = 0.5 / 674500

freq(240000000)

i2c = I2C(0, sda=Pin(43), scl=Pin(44))
i2c.scan() # is this necessary? didn't seem to work without it...

scale_adc = NAU7802(i2c)
scale_lock = asyncio.Lock()
scale_value = Observable(None)
scale_unit = Observable(0)

async def zero_scale():
    scale_value.value = None
    await scale_adc.calibrate("INTERNAL")
    await scale_adc.calibrate("OFFSET")

async def sensor_task():
    while True:
        t0 = time.ticks_ms()
        async with scale_lock:
            raw_value = scale_adc.read()
            weight_in_kg = scale_adc.read() * ONE_KG
            scale_value.value = weight_in_kg
        t1 = time.ticks_ms()
        # weight_in_g_rounded = round(weight_in_kg * 1000)
        dt1 = time.ticks_diff(t1, t0)
        blue.update_weight(weight_in_kg)
        await asyncio.sleep_ms(1)

async def zero_task():
    while True:
        await blue.zero_characteristic.written()
        async with scale_lock:
            await zero_scale()
        blue.zeroed_characteristic.write("a", send_update=True)

async def start_scale():
    async with scale_lock:
        await scale_adc.init()
        await zero_scale()

enc_indev = None
back_indev = None

def setup_input(enc_group, back_group):
    global enc_indev, back_indev
    p1 = Pin(1, mode=Pin.IN, pull=Pin.PULL_UP)
    p2 = Pin(2, mode=Pin.IN, pull=Pin.PULL_UP)
    enc = QDec(p1, p2)
    # enc = Encoder(p1, p2, delay=5)
    last_enc_count = 0
    enc_sw = Pin(3, mode=Pin.IN, pull=Pin.PULL_UP)
    def enc_cb(_driver, data):
        nonlocal last_enc_count
        input_data = lv.indev_data_t.__cast__(data)
        input_data.state = lv.INDEV_STATE.RELEASED if enc_sw() else lv.INDEV_STATE.PRESSED
        # don't want it changing under us
        count = enc.count
        # count = enc.value()
        # weird order for the subtraction, but this reversing leads to a more natural order
        # print(p1(), p2())
        enc_diff = last_enc_count - count
        # if enc_diff != 0:
        #     print(f"{enc_diff=}")
        input_data.enc_diff = enc_diff
        last_enc_count = count
    enc_indev = lv.indev_create()
    enc_indev.set_type(lv.INDEV_TYPE.ENCODER)
    enc_indev.set_read_cb(enc_cb)
    enc_indev.set_group(enc_group)

    back_sw = Pin(10, mode=Pin.IN, pull=Pin.PULL_UP)
    def back_cb(_driver, data):
        input_data = lv.indev_data_t.__cast__(data)
        input_data.key = lv.KEY.ESC
        input_data.state = lv.INDEV_STATE.RELEASED if back_sw() else lv.INDEV_STATE.PRESSED
    back_indev = lv.indev_create()
    back_indev.set_type(lv.INDEV_TYPE.KEYPAD)
    back_indev.set_read_cb(back_cb)
    back_indev.set_group(back_group)

def make_unit_roller(scr):
    roller = lv.roller(scr)
    roller.set_options("g\noz", lv.roller.MODE.INFINITE)
    roller.set_visible_row_count(1)
    def handler(e):
        global unit
        nonlocal roller
        if e.code == lv.EVENT.VALUE_CHANGED:
            scale_unit.value = roller.get_selected()
    roller.add_event(handler, lv.EVENT.ALL, None)
    roller.align(lv.ALIGN.RIGHT_MID, 0, -40)
    roller.add_flag(lv.obj.FLAG.EVENT_BUBBLE)
    return roller

icon_menu: IconMenuView
scr: lv.obj
container: lv.obj
outer_scr: lv.obj
selected_page_idx = Observable(0)
cur_recipe: InProgressRecipe = InProgressRecipe(cookie_recipe)

DEBUG = False

class Page:
    root = None
    group = None
    done = None

    def __init__(self, root, group=None):
        self.root = root
        self.group = Observable(lv.group_create()) if group is None else group
        # self.group = lv.group_create() if group is None else group
        # self.group.add_obj(root)
        self.done = asyncio.Event()
        root.add_event(self.handle_esc, lv.EVENT.KEY, None)

    def handle_esc(self, e):
        # print(f"got key {e.get_key()}, esc is {lv.KEY.ESC}")
        if e.get_key() == lv.KEY.ESC:
            self.done.set()
            e.stop_bubbling = 1

pages: list[Page] = []

async def update_page():
    while True:
        # print(f"{selected_page_idx.value=}")
        await selected_page_idx.event.wait()
        if selected_page_idx.value < len(pages):
            container.scroll_to_y(240 * selected_page_idx.value, lv.ANIM.ON)

async def toplevel():
    while True:
        await icon_menu.done.wait()
        icon_menu.done.clear()

        scr.scroll_to_x(150, lv.ANIM.ON)
        page = pages[min(selected_page_idx.value, len(pages) - 1)]
        event = WaitAny((page.group.event, page.done))
        while not page.done.is_set():
            enc_indev.set_group(page.group.value)
            back_indev.set_group(page.group.value)
            await event.wait()
        page.done.clear()
        # print("page done")

        scr.scroll_to_x(0, lv.ANIM.ON)
        lv.group_focus_obj(icon_menu.buttons[selected_page_idx.value])
        enc_indev.set_group(icon_menu.group)
        back_indev.set_group(icon_menu.group)
        # print("reset grouops")

def make_ui(disp) -> asyncio.Task:
    global scr, outer_scr, icon_menu, container, cur_recipe
    bg_style = lv.style_t()
    bg_style.set_bg_color(lv.color_black())
    # bg_style.set_border_color(lv.color_black())
    bg_style.set_border_width(0)
    debug_style = lv.style_t()
    debug_style.set_border_color(lv.color_make(40, 40, 40))
    def my_theme_cb(_th, ob):
        ob.add_style(bg_style, 0)
        if DEBUG:
            ob.add_style(debug_style, 0)
    theme = lv.theme_default_init(
        disp,
        lv.palette_main(lv.PALETTE.BLUE),
        lv.palette_main(lv.PALETTE.CYAN),
        True, # dark mode
        lv.font_montserrat_16,
    )
    my_theme = lv.theme_t()
    my_theme.set_parent(theme)
    my_theme.set_apply_cb(my_theme_cb)
    disp.set_theme(my_theme)
    outer_scr = lv.obj()
    outer_scr.clear_flag(lv.obj.FLAG.SCROLLABLE)
    outer_scr.set_style_pad_all(0, 0)
    scr = lv.obj(outer_scr)
    scr.clear_flag(lv.obj.FLAG.SCROLLABLE)
    scr.set_style_pad_all(0, 0)
    scr.set_size(lv.pct(100), lv.pct(100))
    scr.set_flex_flow(lv.FLEX_FLOW.ROW)
    scr.set_flex_align(lv.FLEX_ALIGN.START, lv.FLEX_ALIGN.START, lv.FLEX_ALIGN.START)
    scr.clear_flag(lv.obj.FLAG.SCROLLABLE)

    menu_items = [
        lv.SYMBOL.AUDIO,
        lv.SYMBOL.VIDEO,
        lv.SYMBOL.LIST,
        lv.SYMBOL.OK,
        lv.SYMBOL.CLOSE,
    ]
    icon_menu = IconMenuView(selected_page_idx, menu_items, parent=scr)
    lv.group_focus_obj(icon_menu.buttons[0])

    setup_input(icon_menu.group, icon_menu.group)

    container = lv.obj(scr)
    container.clear_flag(lv.obj.FLAG.SCROLLABLE)
    container.set_style_pad_all(0, 0)
    container.set_size(536, 240)
    container.set_flex_flow(lv.FLEX_FLOW.COLUMN)
    container.set_flex_align(lv.FLEX_ALIGN.START, lv.FLEX_ALIGN.START, lv.FLEX_ALIGN.START)
    container.set_style_pad_row(0, 0)

    fs_drv = lv.fs_drv_t()
    fs_driver.fs_register(fs_drv, 'S')
    font = lv.font_load("S:/fonts/FiraSans-SemiBold-digits-128.bin")
    font_small = lv.font_load("S:/fonts/FiraSans-SemiBold-digits-64.bin")
    font_alpha = lv.font_load("S:/fonts/FiraSans-Regular-ascii-36.bin")

    tasks = []

    for i in range(5):
        if i == 0:
            view = RecipeView(scale_value, scale_unit, cur_recipe, parent=container, font_digits_large=font, font_digits_small=font_small, font_text=font_alpha)
            page = Page(view.root, group=view.group)
            tasks.append(asyncio.create_task(view.render()))
        # if i == 0:
        #     view = IngredientsView(parent=container, font=font_alpha)
        #     page = Page(view.root, group=view.group)
        #     print("created ingredients")
        # elif i == 1:
        #     view = WeighingIngredientView(scale_value, scale_unit, cur_recipe.recipe.ingredients[0], cur_recipe.progress[0], parent=container, font_digits_large=font, font_digits_small=font_small, font_text=font_alpha)
        #     page = Page(view.root)
        #     page.group.add_obj(view.root)
        #     tasks.append(asyncio.create_task(view.render()))
        else:
            view = WeightValueView(scale_value, scale_unit, parent=container, font=font, font_small=font_small)
            page = Page(view.root)
            tasks.append(asyncio.create_task(view.render()))
            input_obj = make_unit_roller(view.root)
            page.group.value.add_obj(input_obj)

        pages.append(page)

    # print("created all the pages")

    lv.scr_load(outer_scr)
    return asyncio.create_task(asyncio.gather(update_page(), toplevel(), *tasks))
    

async def main():
    time.sleep(2)
    # print("hi from main")
    # micropython.mem_info()
    display_buf = display.allocate()
    scale_task = asyncio.create_task(start_scale())
    # make sure the scale task starts here...
    # await asyncio.sleep_ms(1)
    # this isn't actually async, but it starts some tasks. do it in
    # parallel here for when i actually make display init async
    disp, disp_driver = await display.start_graphics(display_buf)
    # print("graphics started")
    t4 = make_ui(disp_driver)
    # print("made ui")
    await scale_task
    t1 = asyncio.create_task(sensor_task())
    t2 = asyncio.create_task(blue.peripheral_task())
    t3 = asyncio.create_task(zero_task())
    if aiorepl:
        repl = asyncio.create_task(aiorepl.task())
        await asyncio.gather(t1, t2, t3, t4, repl)
    else:
        await asyncio.gather(t1, t2, t3, t4)

try:
    asyncio.run(main())
finally:
    if lv.is_initialized():
        lv.deinit()
