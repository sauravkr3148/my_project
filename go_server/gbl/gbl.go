package gbl

import (
	jsonstore "cpc/jsonparser"
	"sync"

	"github.com/gofiber/contrib/websocket"
)

var Store jsonstore.JSONStore

var Clients = make(map[string]map[string]map[string][]*websocket.Conn)
var ReverseClients = make(map[string]map[string]map[string]chan []byte)
var PendingHTTPRequests = make(map[string]chan map[string]interface{})
var ClientsMutex sync.RWMutex
var ReverseClientsMutex sync.RWMutex
var PendingHTTPRequestsMutex sync.RWMutex
