#pragma once
#include <string>

/**
 * Prepends the Chivalry 2 Server Browser Backend url to the provided path.
 *
 * The Backend url is retrieved from g_state.
 *
 * @param path
 * @return
 */
std::wstring GetServerBrowserBackendApiUrl(const wchar_t* path);

/**
 * Sends an HTTP GET request to the provided url, returning the response body as a string.
 *
 * No headers are returned in this. It is very basic and implements minimal HTTP Get functionality.
 *
 * @param url
 * @return
 */
std::wstring HTTPGet(const std::wstring* url);