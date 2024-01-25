import asyncio
from primitives import WaitAny

import lvgl as lv

from observable import Observable
from recipe import InProgressRecipe
from .ingredients import IngredientsView
from .weighprogress import WeighingIngredientView


ingredients = [
    "chocolate",
    "all-purpose flour",
    "unsalted butter",
    "white sugar",
    "light brown sugar",
    "vanilla extract",
    "kosher salt",
    "baking soda",
    "baking powder",
    "nutmeg",
    "large egg",
]

SIZE = [536, 240]
BAR_HEIGHT = 128
INGREDIENTS_WIDTH = 536

def print_cb(s):
    def cb(e):
        print(s)
    return cb

class RecipeView:
    def __init__(self, weight: Observable, unit: Observable, recipe: InProgressRecipe, parent=None, font_digits_large=lv.font_montserrat_48, font_digits_small=lv.font_montserrat_16, font_text=lv.font_montserrat_48):
        self.recipe = recipe
        self.weighing_focused = Observable(False)

        self.root = lv.obj(parent)
        self.root.remove_style_all()
        self.root.set_size(*SIZE)
        self.root.clear_flag(lv.obj.FLAG.SCROLLABLE)
        self.root.set_flex_flow(lv.FLEX_FLOW.ROW)

        self.selected_ingredient = Observable(0)

        self.ingredients_view = IngredientsView(recipe.recipe.ingredients, recipe.progress, self.selected_ingredient, parent=self.root, font=font_text)
        self.ingredients_view.root.set_size(INGREDIENTS_WIDTH, 240)

        self.group = Observable(self.ingredients_view.group)

        self.weighing_cont = lv.obj(self.root)
        self.weighing_cont.remove_style_all()
        self.weighing_cont.set_size(*SIZE)
        self.weighing_cont.clear_flag(lv.obj.FLAG.SCROLLABLE)
        self.weighing_cont.set_flex_flow(lv.FLEX_FLOW.COLUMN)

        self.weighing_views = []
        for ingredient, status in zip(recipe.recipe.ingredients, recipe.progress):
            view = WeighingIngredientView(weight, unit, ingredient, status, parent=self.weighing_cont, font_digits_large=font_digits_large, font_digits_small=font_digits_small, font_text=font_text)
            self.weighing_views.append(view)
            break
        
    async def render(self):
        await asyncio.gather(
            self.update_ingredient_clicked(),
            self.update_statuses(),
            self.update_current_ingredient(),
            self.update_weighing_focused(),
            self.ingredients_view.render(),
            *(view.render() for view in self.weighing_views)
        )

    async def update_ingredient_clicked(self):
        while True:
            print("updating weighing_focused")
            self.weighing_focused.value = self.ingredients_view.clicked.is_set()
            self.ingredients_view.clicked.clear()
            await self.ingredients_view.clicked.wait()

    async def update_statuses(self):
        while True:
            # TODO: see if we can store the previous value and thus
            # not iterate through each one... I doubt this is a
            # performance bottleneck though
            selected = self.selected_ingredient.value
            for i, view in enumerate(self.weighing_views):
                view.should_update_status = i == selected
                view.visible = selected - 1 <= i <= selected + 1

            await self.selected_ingredient.event.wait()

    async def update_current_ingredient(self):
        while True:
            # print(f"{self.selected_ingredient.value=}")
            self.weighing_cont.scroll_to_y(240 * self.selected_ingredient.value, lv.ANIM.ON)

            await self.selected_ingredient.event.wait()

    async def update_weighing_focused(self):
        while True:
            self.root.scroll_to_x(INGREDIENTS_WIDTH if self.weighing_focused.value else 0, lv.ANIM.ON)
            await self.weighing_focused.event.wait()

    async def update_group(self):
        event = WaitAny((self.weighing_focused.event, self.selected_ingredient.event))
        while True:
            # if self.weighing_focused.value:
            #     self.group.value = self.weighing_views[self.selected_ingredient.value].group
            # else:
            #     self.group.value = self.ingredients_view.group

            await event.wait()

            
