import time

real_start = time.ticks_ms()

from collections import OrderedDict

ticks = OrderedDict()

ticks['real_start'] = real_start
ticks['start'] = time.ticks_ms()

from machine import Pin, I2C, freq
from nau7802 import NAU7802
import asyncio
import lvgl as lv
try:
    import aiorepl
except ImportError:
    aiorepl = None

ticks['ext_imports'] = time.ticks_ms()

import blue
import display
from observable import Observable

ticks['int_imports'] = time.ticks_ms()

ONE_KG = 0.5 / 674500

freq(240000000)

# TODO: make display initialization async

i2c = I2C(0, sda=Pin(43), scl=Pin(44))
i2c.scan() # is this necessary? didn't seem to work without it...

i2c_scanned = time.ticks_ms()

scale_adc = NAU7802(i2c)
scale_lock = asyncio.Lock()
scale_value = Observable(None)

scale_created = time.ticks_ms()

async def zero_scale():
    scale_value.value = None
    await scale_adc.calibrate("INTERNAL")
    await scale_adc.calibrate("OFFSET")

async def sensor_task():
    while True:
        t0 = time.ticks_ms()
        async with scale_lock:
            raw_value = scale_adc.read()
            print(raw_value)
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

async def scale_value_label_updater(label):
    while True:
        if scale_value.value is None:
            label.set_text("zeroing")
        else:
            weight_in_g = scale_value.value * 1000
            label.set_text(f"{weight_in_g:.2f} g")
        await scale_value.event.wait()

def make_ui():
    scr = lv.obj()
    label = lv.label(scr)
    label.set_style_text_font(lv.font_montserrat_48, 0)
    label.align(lv.ALIGN.CENTER, 0, 0)
    t = asyncio.create_task(scale_value_label_updater(label))
    lv.scr_load(scr)
    return t

async def main():
    ticks['main_start'] = time.ticks_ms()
    scale_task = asyncio.create_task(start_scale())
    # this isn't actually async, but it starts some tasks. do it in
    # parallel here for when i actually make display init async
    disp, buf, disp_driver = await display.start_graphics()
    ticks['graphics_done'] = time.ticks_ms()
    t4 = make_ui()
    ticks['ui_done'] = time.ticks_ms()
    await scale_task
    ticks['zeroing_done'] = time.ticks_ms()
    for name, t in ticks.items():
        print(f"{name}: {t}")
    t1 = asyncio.create_task(sensor_task())
    t2 = asyncio.create_task(blue.peripheral_task())
    t3 = asyncio.create_task(zero_task())
    if aiorepl:
        repl = asyncio.create_task(aiorepl.task())
        await asyncio.gather(t1, t2, t3, t4, repl)
    else:
        await asyncio.gather(t1, t2, t3, t4)

asyncio.run(main())
