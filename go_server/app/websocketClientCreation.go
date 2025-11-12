package relay

import (
	"cpc/gbl"
	"fmt"
	"log"
	"strings"

	"encoding/json"

	"github.com/gofiber/contrib/websocket"
)

func (w websocketRelay) SendConnectedClients() {

	go func() {
		for msg := range w.client_receiver {
			for _, conn := range w.clients {
				if len(w.clients) != 0 && conn != nil {
					if w.screen_transfer {
						if err := conn.WriteMessage(websocket.BinaryMessage, msg); err != nil {
							DebugPrintln("2 Error writing to WebSocket:", err)
							break

						}
					}

				}
			}
		}
	}()
}

func (w websocketRelay) CreateReverseClient(key string, conn *websocket.Conn, tenent_id string, agentType string) {
	ch := make(chan []byte, 100)
	revAgentRecevier := make(chan []byte, 50)
	running := true

	w.registerReverseClient(key, revAgentRecevier, tenent_id, agentType)
	broadcastDisconnect := func(reason string, err error) {

		msg := map[string]interface{}{
			"type":      "agent_disconnected",
			"tenant_id": tenent_id,
			"agentType": agentType,
			"key":       key,
			"reason":    reason,
		}
		w.broadcastChatMessage(msg)

		disconnectPacket := []byte{0x63}
		if clients, ok := gbl.Clients[tenent_id][agentType][key]; ok {
			for _, c := range clients {
				if c != nil {
					c.WriteMessage(websocket.BinaryMessage, disconnectPacket)
				}
			}
		}
	}
	cleanup := func() {
		DebugPrintf("Cleaning up reverse client - TenantID: %s, AgentType: %s, Key: %s\n", tenent_id, agentType, key)
		running = false
		w.RemoveReverseClient(tenent_id, key, agentType)
	}

	go func() {
		defer func() {
			if r := recover(); r != nil {
				DebugPrintln("Recovered from panic:", r)
				broadcastDisconnect("panic", fmt.Errorf("%v", r))
				log.Printf("reverse client disconnected due to panic")
				cleanup()
			}
		}()
		for {
			if !running {
				break
			}
			messageType, message, err := conn.ReadMessage()
			if err != nil {
				DebugPrintln("Read error:", err)
				broadcastDisconnect("read error", err)
				cleanup()
				break
			}
			if messageType == websocket.TextMessage {
				cleanedMessage := strings.ReplaceAll(string(message), "\x00", "")
				cleanedMessage = strings.TrimSpace(cleanedMessage)
				if len(cleanedMessage) > 0 {
					var parsedMessage map[string]interface{}
					if err := json.Unmarshal([]byte(cleanedMessage), &parsedMessage); err == nil {
						if requestID, exists := parsedMessage["request_id"]; exists {
							requestIDStr := fmt.Sprintf("%v", requestID)
							gbl.PendingHTTPRequestsMutex.Lock()
							if ch, exists := gbl.PendingHTTPRequests[requestIDStr]; exists {
								select {
								case ch <- parsedMessage:
									DebugPrintf("Successfully sent response to HTTP handler for request_id: %s\n", requestIDStr)
								default:
									DebugPrintf("HTTP response channel blocked for request_id: %s\n", requestIDStr)
								}
								delete(gbl.PendingHTTPRequests, requestIDStr)
								gbl.PendingHTTPRequestsMutex.Unlock()
								continue
							}
							gbl.PendingHTTPRequestsMutex.Unlock()
						}
						if msgType, exists := parsedMessage["type"]; exists {
							switch msgType {
							case "chat_message":
								parsedMessage["source"] = "chat_agent"
								parsedMessage["from"] = tenent_id
								if uuid, hasUUID := parsedMessage["uuid"]; hasUUID {
									parsedMessage["uuid"] = uuid
								}
								w.broadcastChatMessage(parsedMessage)
							}
						}
					}
				}
			} else {
				ch <- message
			}

		}
	}()

	go func() {
		defer func() {
			if r := recover(); r != nil {
				DebugPrintln("Recovered from panic in revAgentRecevier goroutine:", r)
				cleanup()
			}
		}()
		for {
			if !running {
				break
			}
			packet, ok := <-revAgentRecevier
			if !ok {
				DebugPrintln("revAgentRecevier channel closed")
				cleanup()
				break
			}
			if json.Valid(packet) {
				if err := conn.WriteMessage(websocket.TextMessage, packet); err != nil {
					DebugPrintln("Error writing TextMessage to agent:", err)
					cleanup()
					break
				}
			} else {
				if err := conn.WriteMessage(websocket.BinaryMessage, packet); err != nil {
					DebugPrintln("Error writing BinaryMessage to agent:", err)
					cleanup()
					break
				}
			}

		}

	}()

	defer func() {
		if r := recover(); r != nil {
			DebugPrintln("Recovered from panic:", r)
			DebugPrintln("reverse client disconnected due to panic")
			cleanup()
		}
	}()

	for {
		if !running {
			break
		}
		packet := <-ch
		connections, exists := gbl.Clients[tenent_id][agentType][key]
		if exists {
			for index, conn := range connections {
				if conn != nil {
					if err := conn.WriteMessage(websocket.BinaryMessage, packet); err != nil {

						if index == 0 {
							DebugPrintln("Write error (removing client):", err)
						}
						broadcastDisconnect("write error", err)
						w.RemoveClient(tenent_id, key, index, agentType)
						continue
					}
				}
			}
		}
	}

}

func (w websocketRelay) createClient(rawKey string, conn *websocket.Conn, tenant_id string, agentType string) {
	running := true

	DebugPrintf("createClient - Original rawKey: %s\n", rawKey)

	privateKey := gbl.Store.GetPrivateKeyUsingPublicKey(rawKey)
	if privateKey == "" {
		DebugPrintf("createClient: store has no mapping yet for %s — falling back to rawKey\n", rawKey)
		privateKey = rawKey
	} else {
		DebugPrintf("createClient - Transformed privateKey: %s\n", privateKey)
	}

	key := privateKey
	w.registerClient(key, conn, tenant_id, agentType)
	DebugPrintf("createClient: registered client for tenant=%s agentType=%s key=%s\n", tenant_id, agentType, key)

	for running {
		messageType, message, err := conn.ReadMessage()
		if err != nil {
			DebugPrintf("createClient Read error: %v\n", err)
			break
		}

		if messageType == websocket.TextMessage {
			cleanedMessage := strings.ReplaceAll(string(message), "\x00", "")
			cleanedMessage = strings.TrimSpace(cleanedMessage)
			if len(cleanedMessage) > 0 && json.Valid([]byte(cleanedMessage)) {
				var parsedMessage map[string]interface{}
				if err := json.Unmarshal([]byte(cleanedMessage), &parsedMessage); err == nil {
					parsedMessage["tenant_id"] = tenant_id

					if _, hasFrom := parsedMessage["from"]; !hasFrom {
						parsedMessage["from"] = key
					}
					parsedMessage["agent_type"] = agentType

					if msgType, exists := parsedMessage["type"]; exists {
						switch msgType {
						case "chat_message":
							parsedMessage["source"] = "javascript_client"

							w.broadcastChatMessage(parsedMessage)
							continue
						case "status_update":
							w.broadcastStatusUpdate(parsedMessage)
							continue
						}
					}
				} else {
					DebugPrintf("createClient JSON unmarshal error: %v\n", err)
				}
			}
		}

		gbl.ReverseClientsMutex.RLock()
		if connection, exists := gbl.ReverseClients[tenant_id][agentType][key]; exists {
			gbl.ReverseClientsMutex.RUnlock()
			select {
			case connection <- message:
			default:
				DebugPrintf("createClient: reverse client channel blocked for %s/%s/%s\n", tenant_id, agentType, key)
			}
		} else {
			gbl.ReverseClientsMutex.RUnlock()
			DebugPrintf("No reverse client found for %s/%s/%s — available reverse keys: %v\n",
				tenant_id, agentType, key, getKeysForTenantAndAgent(tenant_id, agentType))
		}
	}
}

func (w websocketRelay) handleNewConnection(c *websocket.Conn) {
	key := c.Params("key")
	id := c.Params("tenant_id")
	agentType := c.Params("agent_type")

	DebugPrintf("New connection - Type: %s, AgentType: %s, TenantID: %s, Key: %s\n",
		c.Params("type"), agentType, id, key)

	if c.Params("type") == "rev" {
		DebugPrintf("Creating reverse client (agent) with key: %s\n", key)
		w.CreateReverseClient(key, c, id, agentType)
	} else if c.Params("type") == "cli" {
		DebugPrintf("Creating regular client with key: %s\n", key)
		w.createClient(key, c, id, agentType)
	} else {
		DebugPrintf("Unknown connection type: %s\n", c.Params("type"))
	}
}
