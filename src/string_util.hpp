#pragma once

#include <vector>
#include <string>

inline auto quot = "\"";

/**
 * Splits a string in to a vector of parts by some delimiter
 *
 * @param str
 * @param delimiter
 * @return
 */
std::vector<std::string> split(std::string_view str, std::string_view delimiter);

/**
 * Creates whitespace, beginning with a newline, indented 2 spaces for each indent level.
 *
 * @param indent
 * @return
 */
std::string ws(int indent);