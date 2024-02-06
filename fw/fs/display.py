import lcd
import time
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
    (0x35, b'\x00', 0),   # Set Tearing Effect signal to only output at Vblank
)

class VSyncDisplay:
    def __init__(self, display, te):
        self.display = display
        self.go = 0
        if te is not None:
            te.irq(trigger=Pin.IRQ_RISING, handler=self._intr)

    def _intr(self, _):
        self.go = 1

    def bitmap(self, *args):
        # wait for vblank
        self.go = 0
        while self.go == 0: pass

        self.display.bitmap(*args)

    def __getattr__(self, attr):
        return getattr(self.display, attr)

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
    # return VSyncDisplay(rm, Pin(9))
    return rm

def allocate(factor=8):
    return bytearray((240 * 536 * 2) // factor)

def set_up_lvgl(buf, disp, factor=8):
    if not lv.is_initialized():
        lv.init()
    disp_drv = lv.disp_create(disp.width(), disp.height())
    disp_drv.set_color_format(lv.COLOR_FORMAT.NATIVE_REVERSED)
    buf1 = allocate(factor=1)
    # buf1 = buf
    # buf1 = allocate(factor=10)
    # buf2 = allocate(factor=1)
    # buf2 = None
    buf2 = None
    disp_drv.set_draw_buffers(buf1, buf2, len(buf1), lv.DISP_RENDER_MODE.FULL)
    prev_flush = time.ticks_ms()
    def flush(drv, area, color_p):
        nonlocal prev_flush
        x1, x2, y1, y2 = area.x1, area.x2, area.y1, area.y2
        size = (x2 - x1 + 1) * (y2 - y1 + 1) * 2
        # wtf is this "dereference"? idk, just copying from the example drivers...
        color_view = color_p.__dereference__(size)
        this_flush = time.ticks_ms()
        # print(f"time since last flush: {time.ticks_diff(this_flush, prev_flush)}")
        prev_flush = this_flush
        disp.bitmap(x1, y1, x2 + 1, y2 + 1, color_view)
        after = time.ticks_ms()
        # print(f"flush took {time.ticks_diff(after, this_flush)}ms")
        drv.flush_ready()
    disp_drv.set_flush_cb(flush)
    return disp_drv

async def lv_refresh(refresh_event: asyncio.Event):
    while True:
        await refresh_event.wait()
        if lv._nesting.value == 0:
            refresh_event.clear()
            before = time.ticks_ms()
            lv.task_handler() # don't handle any exceptions here
            after = time.ticks_ms()
            # print(f"handler took {time.ticks_diff(after, before)}ms")

async def lv_ticker(refresh_event: asyncio.Event, period: int):
    last_time = time.ticks_ms()
    while True:
        await asyncio.sleep_ms(period)
        now = time.ticks_ms()
        since_last_tick = time.ticks_diff(now, last_time)
        # print(f"{since_last_tick}ms between ticks")
        lv.tick_inc(since_last_tick)
        refresh_event.set()
        last_time = now

async def start_graphics(buf, rotation: int = 1, period: int = 33):
    disp = config(rotation=rotation)
    drv = set_up_lvgl(buf, disp)
    refresh_event = asyncio.Event()
    asyncio.create_task(lv_refresh(refresh_event))
    asyncio.create_task(lv_ticker(refresh_event, period))
    return disp, drv
