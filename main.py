import tft_config
import logo
import time
import framebuf_plus
from FiraSansBold32pt import FiraSansBold32pt as GFXFont
import lcd
from machine import Pin, SPI, I2C, freq
from nau7802 import NAU7802
import uasyncio as asyncio

import blue


ONE_KG = 0.141 / 189800

def my_tft_config():
    hspi = SPI(2, sck=Pin(47), mosi=None, miso=None, polarity=0, phase=0)
    panel = lcd.QSPIPanel(
        spi=hspi,
        data=(Pin(18), Pin(7), Pin(48), Pin(5)),
        command=Pin(7),
        cs=Pin(6),
        pclk=80 * 1000 * 1000,
        width=240,
        height=536
    )
    rm = lcd.RM67162(panel, reset=Pin(17), bpp=24)
    rm.reset()
    rm.init()
    # rm.custom_init(rm67162_qspi_init)
    rm.rotation(0)
    rm.backlight_on()
    return rm

freq(240000000)

disp = my_tft_config()
disp.rotation(3)

i2c = I2C(0, sda=Pin(43), scl=Pin(44))
i2c.scan() # is this necessary? didn't seem to work without it...

scale_adc = NAU7802(i2c)
scale_adc.enable(True)

def zero_scale():
    global scale_adc
    scale_adc.calibrate("INTERNAL")
    scale_adc.calibrate("OFFSET")

zero_scale()

buffer = bytearray(disp.width() * disp.height() * 3)
fb = framebuf_plus.FrameBuffer(buffer, disp.width(), disp.height(), framebuf_plus.RGB888)
fb.gfx(GFXFont)

async def sensor_task():
    while True:
        t0 = time.ticks_ms()
        weight_in_kg = scale_adc.read() * ONE_KG
        t1 = time.ticks_ms()
        fb.fill(0x000000)
        t2 = time.ticks_ms()
        weight_in_g_rounded = round(weight_in_kg * 1000)
        fb.write(str(weight_in_g_rounded), 0, 200, (0xfffffff, 0x000000))
        t3 = time.ticks_ms()
        disp.bitmap(0, 0, logo.WIDTH, logo.HEIGHT, buffer)
        t4 = time.ticks_ms()
        dt1 = time.ticks_diff(t1, t0)
        dt2 = time.ticks_diff(t2, t1)
        dt3 = time.ticks_diff(t3, t2)
        dt4 = time.ticks_diff(t4, t3)
        print(f"{dt1}, {dt2}, {dt3}, {dt4}")
        blue.update_weight(weight_in_kg)
        await asyncio.sleep_ms(1)

async def zero_task():
    while True:
        await blue.zero_characteristic.written()
        zero_scale()
        blue.zeroed_characteristic.write("a", send_update=True)

async def main():
    t1 = asyncio.create_task(sensor_task())
    t2 = asyncio.create_task(blue.peripheral_task())
    t3 = asyncio.create_task(zero_task())
    await asyncio.gather(t1, t2, t3)


asyncio.run(main())
