import asyncio
from primitives import WaitAny

import lvgl as lv

from observable import Observable
from recipe import Ingredient, InProgressIngredient


SIZE = [536, 240]

def print_cb(s):
    def cb(e):
        print(s)
    return cb

class IngredientsView:
    def __init__(self, ingredients: list[Ingredient], ingredient_statuses: list[Observable], selected: Observable, parent=None, font=lv.font_montserrat_48):
        self.ingredients = ingredients
        self.ingredient_statuses = ingredient_statuses
        self.selected = selected
        self.root = lv.list(parent)
        self.root.set_size(*SIZE)
        self.root.set_scroll_snap_y(lv.SCROLL_SNAP.CENTER)
        self.clicked = asyncio.Event()
        self.group = lv.group_create()
        button_style = lv.style_t()
        button_style.set_pad_all(10)
        self.labels = []
        for i, ingredient in enumerate(ingredients):
            btn = self.root.add_btn(None, ingredient.name)
            self.group.add_obj(btn)
            btn.add_event(self.selected.update_to(i), lv.EVENT.FOCUSED, None)
            btn.add_event(self.click_cb, lv.EVENT.CLICKED, None)
            btn.add_style(button_style, 0)
            label = btn.get_child(0)
            self.labels.append(label)
            label.set_style_text_font(font, 0)

    def click_cb(self, e):
        print("ing button clicked")
        self.clicked.set()

    async def render(self):
        await self.render_dones()

    async def render_dones(self):
        update_event = WaitAny([ingr.event for ingr in self.ingredient_statuses])
        while True:
            for ingr, label in zip(self.ingredient_statuses, self.labels):
                decor = lv.TEXT_DECOR.STRIKETHROUGH if ingr.value.done else lv.TEXT_DECOR.NONE
                label.set_style_text_decor(decor, 0)

            await update_event.wait()
