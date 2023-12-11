from micropython import const

try:
    import asyncio
except ImportError:
    import uasyncio as asyncio
import aioble
import bluetooth

import struct


_WEIGHT_STRUCT = "<f"
_


def _encode_weight(w):
    return struct.pack(_WEIGHT_STRUCT, w)

_SCALE_SERVICE_UUID = bluetooth.UUID('8e639146-13cd-4c18-b6d2-1d0918cb611f')

_WEIGHT_CHARACTERISTIC_UUID = bluetooth.UUID('0335b3cd-f55b-49ed-b5f4-f641acab1b16')
_MAX_WEIGHT_CHARACTERISTIC_UUID = bluetooth.UUID('b602629f-f0a6-40a3-9eeb-30f0808712f7')
_ZERO_CHARACTERISTIC_UUID = bluetooth.UUID('33f2eaa8-d11e-493c-9f05-1f3cbf826f2c')
_ZEROED_CHARACTERISTIC_UUID = bluetooth.UUID('5cce657e-13bb-46dc-8fc1-1129463a3d93')

_SCALE_APPEARANCE = const(0x0c80)

# How frequently to send advertising beacons.
_ADV_INTERVAL_US = 250_000


# Register GATT server.
scale_service = aioble.Service(_SCALE_SERVICE_UUID)
weight_characteristic = aioble.Characteristic(
    scale_service, _WEIGHT_CHARACTERISTIC_UUID, read=True, notify=True
)
max_weight_characteristic = aioble.Characteristic(
    scale_service, _MAX_WEIGHT_CHARACTERISTIC_UUID, read=True, initial=_encode_weight(2.5)
)
zero_characteristic = aioble.Characteristic(
    scale_service, _ZERO_CHARACTERISTIC_UUID, write=True
)
zeroed_characteristic = aioble.Characteristic(
    scale_service, _ZEROED_CHARACTERISTIC_UUID, indicate=True
)
aioble.register_services(scale_service)


def update_weight(w):
    print(f"updating weight, {weight_characteristic._value_handle is not None}")
    weight_characteristic.write(_encode_weight(w), send_update=True)

# Serially wait for connections. Don't advertise while a central is
# connected.
async def peripheral_task():
    while True:
        async with await aioble.advertise(
            _ADV_INTERVAL_US,
            name="jessies-scale",
            services=[_SCALE_SERVICE_UUID],
            appearance=_SCALE_APPEARANCE,
        ) as connection:
            print("Connection from", connection.device)
            await connection.disconnected(timeout_ms=None)
