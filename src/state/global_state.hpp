#pragma once
#include "State.hpp"

/**
 * This should only be accessed when there is no other way to get what you need.
 *
 * Thus far, this is only needed inside hooked methods.  Any other code ought to have a better way to get access to the
 * state that doesn't involve accessing the global reference.
 *
 * Minimizing the surface area of access to g_state makes it much easier to reason about what is going on.
 */
inline State* g_state;
void initialize_global_state(State* state);