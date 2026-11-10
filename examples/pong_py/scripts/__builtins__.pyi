# Editor support only: the engine gives every script loaded through
# `sage_ecs_py::Ecs::load` a `world` global. Pyright and Pylance add the names
# in this file to builtins for scripts in this directory.

import sage

world: sage.World
