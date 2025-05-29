#include "constants.h"
#include "nettools.h"
#include "logging/global_logger.hpp"
#include <windows.h>
#include <vector>
#include <winhttp.h>
#include <stringapiset.h>

#include "state/global_state.hpp"

const wchar_t* Utf8ToTChar(const char* utf8bytes)
{
    static thread_local std::vector<wchar_t> buffer;
    
    int bufferSize = MultiByteToWideChar(CP_UTF8, 0, utf8bytes, -1, nullptr, 0);
    buffer.resize(bufferSize);
    MultiByteToWideChar(CP_UTF8, 0, utf8bytes, -1, buffer.data(), bufferSize);
    
    return buffer.data();
}

std::wstring GetApiUrl(const wchar_t* path) {
	return g_state->GetCLIArgs().server_browser_backend + path;
}

std::wstring HTTPGet(const std::wstring* url) {
	std::wstring response = L"";

	URL_COMPONENTSW lpUrlComponents = { 0 }; // Initialize the structure to zero.
	lpUrlComponents.dwStructSize = sizeof(URL_COMPONENTSW);
	lpUrlComponents.dwSchemeLength = (DWORD)-1;    // Let WinHttpCrackUrl allocate memory.
	lpUrlComponents.dwHostNameLength = (DWORD)-1;  // Let WinHttpCrackUrl allocate memory.
	lpUrlComponents.dwUrlPathLength = (DWORD)-1;   // Let WinHttpCrackUrl allocate memory.

	// TODO: these are probably allocated unnecessarily.
	// Previous statements suggest that the Crack call will
	// allocate those buffers itself

	// Allocate buffers for the URL components
	wchar_t* schemeBuf = new wchar_t[url->length() + 1];
	wchar_t* hostNameBuf = new wchar_t[url->length() + 1];
	wchar_t* urlPathBuf = new wchar_t[url->length() + 1];

	// Assign buffers to the structure
	lpUrlComponents.lpszScheme = schemeBuf;
	lpUrlComponents.lpszHostName = hostNameBuf;
	lpUrlComponents.lpszUrlPath = urlPathBuf;

	bool success = WinHttpCrackUrl(url->c_str(), url->length(), 0, &lpUrlComponents);

	if (!success) {
		GLOG_ERROR("Failed to crack URL");
		DWORD error = GetLastError();

		switch (error)
		{
		case ERROR_WINHTTP_INTERNAL_ERROR:
			GLOG_ERROR("ERROR_WINHTTP_INTERNAL_ERROR");
			break;
		case ERROR_WINHTTP_INVALID_URL:
			GLOG_ERROR("ERROR_WINHTTP_INVALID_URL");
			break;
		case ERROR_WINHTTP_UNRECOGNIZED_SCHEME:
			GLOG_ERROR("ERROR_WINHTTP_UNRECOGNIZED_SCHEME");
			break;
		case ERROR_NOT_ENOUGH_MEMORY:
			GLOG_ERROR("ERROR_NOT_ENOUGH_MEMORY");
			break;
		default:
			break;
		}

		return response;
	}

	std::wstring host = std::wstring(lpUrlComponents.lpszHostName, lpUrlComponents.dwHostNameLength);
	std::wstring path = std::wstring(lpUrlComponents.lpszUrlPath, lpUrlComponents.dwUrlPathLength);
	std::wstring scheme = std::wstring(lpUrlComponents.lpszScheme, lpUrlComponents.dwSchemeLength);
	bool tls = scheme == L"https";
	int port = lpUrlComponents.nPort;

	BOOL bResults = FALSE;
	HINTERNET hSession = NULL, hConnect = NULL, hRequest = NULL;
	DWORD dwSize = 0;
	DWORD dwDownloaded = 0;
	LPSTR pszOutBuffer;

	try {
		// Use WinHttpOpen to obtain a session handle.
		hSession = WinHttpOpen(L"Chivalry 2 Unchained/0.4",
			WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
			WINHTTP_NO_PROXY_NAME,
			WINHTTP_NO_PROXY_BYPASS, 0);

		// Specify an HTTP server.

		if (hSession) {
			hConnect = WinHttpConnect(hSession, host.c_str(), port, 0);
		}
		else {
			GLOG_ERROR("Failed to open WinHttp session");
		}

		// Create an HTTP request handle.
		if (hConnect)
			hRequest = WinHttpOpenRequest(hConnect, L"GET", path.c_str(),
				NULL, WINHTTP_NO_REFERER,
				WINHTTP_DEFAULT_ACCEPT_TYPES,
				tls ? WINHTTP_FLAG_SECURE : 0);
		else
			GLOG_ERROR("Failed to connect to WinHttp target");

		// Send a request.
		if (hRequest)
			bResults = WinHttpSendRequest(hRequest,
				WINHTTP_NO_ADDITIONAL_HEADERS, 0,
				WINHTTP_NO_REQUEST_DATA, 0,
				0, 0);
		else
			GLOG_ERROR("Failed to open WinHttp request");

		// End the request.
		if (bResults)
			bResults = WinHttpReceiveResponse(hRequest, NULL);
		else
			GLOG_ERROR("Failed to send WinHttp request");

		// Keep checking for data until there is nothing left.
		if (bResults) {
			do {
				// Check for available data.
				dwSize = 0;
				if (!WinHttpQueryDataAvailable(hRequest, &dwSize)) {
					GLOG_ERROR("Error %u in WinHttpQueryDataAvailable.\n", GetLastError());
					break;
				}

				// Allocate space for the buffer.
				pszOutBuffer = new char[dwSize + 1];
				if (!pszOutBuffer) {
					GLOG_ERROR("Out of memory");
					dwSize = 0;
					break;
				}
				else {
					// Read the data.
					ZeroMemory(pszOutBuffer, dwSize + 1);

					if (!WinHttpReadData(hRequest, (LPVOID)pszOutBuffer,
						dwSize, &dwDownloaded)) {
						GLOG_ERROR("Error %u in WinHttpReadData.", GetLastError());
					}
					else {
						// Data has been read successfully.
						std::wstring chunk = Utf8ToTChar(pszOutBuffer);
						response.append(chunk);
					}

					// Free the memory allocated to the buffer.
					delete[] pszOutBuffer;
				}
			} while (dwSize > 0);
		}
		else
			GLOG_ERROR("Failed to receive WinHttp response");

		if (!hRequest || !hConnect || !hSession) {
			GLOG_ERROR("Failed to open WinHttp handles");
			std::wstring message =
				L"Host: " + host + L"\n" +
				L"Port: " + std::to_wstring(port) + L"\n" +
				L"Path: " + path + L"\n" +
				L"TLS: " + std::to_wstring(tls);
			GLOG_ERROR("{}", message);
		}
	}
	catch (...) {
		GLOG_ERROR("Exception in HTTPGet");
		delete[] schemeBuf;
		delete[] hostNameBuf;
		delete[] urlPathBuf;
		if (hRequest) WinHttpCloseHandle(hRequest);
		if (hConnect) WinHttpCloseHandle(hConnect);
		if (hSession) WinHttpCloseHandle(hSession);
		throw;
	}
	delete[] schemeBuf;
	delete[] hostNameBuf;
	delete[] urlPathBuf;
	// Close any open handles.
	if (hRequest) WinHttpCloseHandle(hRequest);
	if (hConnect) WinHttpCloseHandle(hConnect);
	if (hSession) WinHttpCloseHandle(hSession);

	return response;
}