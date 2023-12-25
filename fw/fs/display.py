import lcd
from machine import Pin, SPI
import asyncio

import lvgl as lv

rm67162_spi_init = (
    (0xFE, b'\x00', 0),   # PAGE
    (0x36, b'\x00', 0),   # Scan Direction Control
    (0x3A, b'\x75', 0),   # Interface Pixel Format 16bit/pixel
    (0x51, b'\x00', 0),   # Write Display Brightness MAX_VAL=0XFF
    (0x11, b'\x00', 120), # Sleep Out
    (0x29, b'\x00', 120), # Display on
    (0x51, b'\xD0', 0),   # Write Display Brightness MAX_VAL=0XFF
)

rm67162_qspi_init = (
    (0x11, b'\x00', 5),   # Sleep Out
    (0x3A, b'\x75', 0),   # Interface Pixel Format 16bit/pixel
    (0x51, b'\x00', 0),   # Write Display Brightness MAX_VAL=0XFF
    (0x29, b'\x00', 5),   # Display on
    (0x51, b'\xD0', 0),   # Write Display Brightness MAX_VAL=0XFF
)

def config(rotation=0):
    hspi = SPI(2, sck=Pin(47), mosi=None, miso=None, polarity=0, phase=0)
    panel = lcd.QSPIPanel(
        spi=hspi,
        data=(Pin(18), Pin(7), Pin(48), Pin(5)),
        dc=Pin(7),
        cs=Pin(6),
        pclk=80 * 1000 * 1000,
        width=240,
        height=536
    )
    rm = lcd.RM67162(panel, reset=Pin(17), bpp=16)
    rm.reset()
    # rm.init()
    rm.custom_init(rm67162_qspi_init)
    rm.rotation(rotation)
    rm.backlight_on()
    return rm

def set_up_lvgl(disp, factor=4):
    if not lv.is_initialized():
        lv.init()
    disp_drv = lv.disp_create(disp.width(), disp.height())
    disp_drv.set_color_format(lv.COLOR_FORMAT.NATIVE_REVERSED)
    buf = bytearray((disp.width() * disp.height() * 2) // factor)
    disp_drv.set_draw_buffers(buf, None, len(buf), lv.DISP_RENDER_MODE.PARTIAL)
    def flush(drv, area, color_p):
        x1, x2, y1, y2 = area.x1, area.x2, area.y1, area.y2
        size = (x2 - x1 + 1) * (y2 - y1 + 1) * 2
        # wtf is this "dereference"? idk, just copying from the example drivers...
        color_view = color_p.__dereference__(size)
        disp.bitmap(x1, y1, x2 + 1, y2 + 1, color_view)
        drv.flush_ready()
    disp_drv.set_flush_cb(flush)
    return buf, disp_drv

async def lv_refresh(refresh_event: asyncio.Event):
    while True:
        await refresh_event.wait()
        if lv._nesting.value == 0:
            refresh_event.clear()
            lv.task_handler() # don't handle any exceptions here

async def lv_ticker(refresh_event: asyncio.Event, period: int):
    while True:
        await asyncio.sleep_ms(period)
        lv.tick_inc(period)
        refresh_event.set()

async def start_graphics(rotation: int = 3, period: int = 33):
    disp = config(rotation=rotation)
    buf, drv = set_up_lvgl(disp)
    refresh_event = asyncio.Event()
    asyncio.create_task(lv_refresh(refresh_event))
    asyncio.create_task(lv_ticker(refresh_event, period))
    return disp, buf, drv
