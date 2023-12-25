import asyncio


class Observable:
    def __init__(self, value):
        self.event = asyncio.Event()
        self._value = value

    @property
    def value(self):
        return self._value

    @value.setter
    def value(self, new_value):
        self._value = new_value
        self.event.set()
        self.event.clear()
