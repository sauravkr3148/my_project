package relay

import (
	"cpc/gbl"
	"encoding/json"

	"github.com/gofiber/contrib/websocket"
)

func GetRelayServer() RelayServer {
	return NewWebsocketRelayServer()
}

func (w websocketRelay) broadcastChatMessage(message map[string]interface{}) {

	msgBytes, _ := json.Marshal(message)

	toAgent, hasTo := message["to"].(string)
	agentType, hasAgentType := message["agent_type"].(string)
	fromAgent, _ := message["from"].(string)

	tenantID, hasTenant := message["from"].(string)

	if !hasTo || toAgent == "all_clients" {
		agentType = "chat_agent"
	}
	if !hasAgentType {
		agentType = "chat_agent"
	}

	source, hasSource := message["source"].(string)
	isFromJavaScriptClient := hasSource && source == "javascript_client"

	if isFromJavaScriptClient {

		DebugPrintf("Routing message from JavaScript client to Go agent (target: %s)\n", toAgent)

		if hasTo && toAgent != "all_clients" && toAgent != "" {

			targetTenantID := w.findTenantForAgent(toAgent, agentType)
			if targetTenantID == "" {
				DebugPrintf("Could not determine tenant for agent %s\n", toAgent)
				return
			}

			found := false
			if tenantClients, exists := gbl.ReverseClients[targetTenantID]; exists {
				if agentTypeClients, exists := tenantClients[agentType]; exists {
					if channel, exists := agentTypeClients[toAgent]; exists {
						DebugPrintf("Sending chat message to reverse client with original key - tenant: %s, agentType: %s, key: %s\n", targetTenantID, agentType, toAgent)
						select {
						case channel <- msgBytes:
							found = true
						default:
							DebugPrintln("Error: reverse client channel is full or closed")
						}
					}

					if !found {
						transformedKey := gbl.Store.GetPrivateKeyUsingPublicKey(toAgent)
						if transformedKey != toAgent {
							if channel, exists := agentTypeClients[transformedKey]; exists {
								DebugPrintf("Sending chat message to reverse client with transformed key - tenant: %s, agentType: %s, key: %s\n", targetTenantID, agentType, transformedKey)
								select {
								case channel <- msgBytes:
									found = true
								default:
									DebugPrintln("Error: reverse client channel is full or closed")
								}
							}
						}
					}
				}
			}

			if !found {
				DebugPrintf("Target agent '%s' not found in tenant %s\n", toAgent, targetTenantID)
			}
		} else if hasTenant {
			if tenantClients, exists := gbl.ReverseClients[tenantID]; exists {
				if agentTypeClients, exists := tenantClients[agentType]; exists {
					for key, channel := range agentTypeClients {
						DebugPrintf("Broadcasting chat message to reverse client in same tenant - tenant: %s, agentType: %s, key: %s\n", tenantID, agentType, key)
						select {
						case channel <- msgBytes:
						default:
							DebugPrintln("Error: reverse client channel is full or closed")
						}
					}
				}
			}
		}

		if hasTo && toAgent != "all_clients" && toAgent != "" {
			targetTenantID := w.findTenantForAgent(toAgent, agentType)
			if targetTenantID == "" {
				DebugPrintf("Could not determine tenant for agent %s\n", toAgent)
				return
			}

			if !w.isAgentOnline(toAgent, targetTenantID, agentType) {
				DebugPrintf("Target agent %s is offline - not broadcasting to JavaScript clients\n", toAgent)
				return
			}

			if tenantClients, exists := gbl.Clients[targetTenantID]; exists {
				if agentTypeClients, exists := tenantClients[agentType]; exists {
					transformedKey := gbl.Store.GetPrivateKeyUsingPublicKey(toAgent)
					keysToCheck := []string{toAgent}
					if transformedKey != toAgent {
						keysToCheck = append(keysToCheck, transformedKey)
					}

					for _, keyToCheck := range keysToCheck {
						if connections, exists := agentTypeClients[keyToCheck]; exists {
							DebugPrintf("Broadcasting message from JS client to other JS clients connected to agent %s - tenant: %s, agentType: %s, key: %s\n", toAgent, targetTenantID, agentType, keyToCheck)
							for _, conn := range connections {
								if conn != nil {
									if err := conn.WriteMessage(websocket.TextMessage, msgBytes); err != nil {
										DebugPrintln("Error broadcasting message to JS client:", err)
									}
								}
							}
							break
						}
					}
				}
			}
		} else if hasTenant {
			if tenantClients, exists := gbl.Clients[tenantID]; exists {
				if agentTypeClients, exists := tenantClients[agentType]; exists {
					for key, connections := range agentTypeClients {
						DebugPrintf("Broadcasting message from JS client to all JS clients in same tenant - tenant: %s, agentType: %s, key: %s\n", tenantID, agentType, key)
						for _, conn := range connections {
							if conn != nil {
								if err := conn.WriteMessage(websocket.TextMessage, msgBytes); err != nil {
									DebugPrintln("Error broadcasting message to JS client:", err)
								}
							}
						}
					}
				}
			}
		}
	} else {
		DebugPrintf("Broadcasting message from Go agent (%s) to JavaScript clients\n", fromAgent)

		tenantID, hasTenant := message["from"].(string)
		if !hasTenant {
			DebugPrintln("No tenantID found in message; cannot broadcast")
			return
		}

		if tenantClients, exists := gbl.Clients[tenantID]; exists {
			if agentTypeClients, exists := tenantClients[agentType]; exists {
				for _, connections := range agentTypeClients {
					for _, conn := range connections {
						if conn != nil {
							if err := conn.WriteMessage(websocket.TextMessage, msgBytes); err != nil {
								DebugPrintln("Error broadcasting chat message to JS client:", err)
							} else {
								DebugPrintln("Message sent to JS client successfully")
							}
						}
					}
				}
			}
		}
	}

}

func (w websocketRelay) findTenantForAgent(agentKey, agentType string) string {
	gbl.ReverseClientsMutex.RLock()
	for tenantID, tenantClients := range gbl.ReverseClients {
		if agentTypeClients, exists := tenantClients[agentType]; exists {
			if _, exists := agentTypeClients[agentKey]; exists {
				gbl.ReverseClientsMutex.RUnlock()
				return tenantID
			}
			transformedKey := gbl.Store.GetPrivateKeyUsingPublicKey(agentKey)
			if transformedKey != agentKey {
				if _, exists := agentTypeClients[transformedKey]; exists {
					gbl.ReverseClientsMutex.RUnlock()
					return tenantID
				}
			}
		}
	}
	gbl.ReverseClientsMutex.RUnlock()

	gbl.ClientsMutex.RLock()
	for tenantID, tenantClients := range gbl.Clients {
		if agentTypeClients, exists := tenantClients[agentType]; exists {
			if _, exists := agentTypeClients[agentKey]; exists {
				gbl.ClientsMutex.RUnlock()
				return tenantID
			}
			transformedKey := gbl.Store.GetPrivateKeyUsingPublicKey(agentKey)
			if transformedKey != agentKey {
				if _, exists := agentTypeClients[transformedKey]; exists {
					gbl.ClientsMutex.RUnlock()
					return tenantID
				}
			}
		}
	}
	gbl.ClientsMutex.RUnlock()

	return ""
}

func (w websocketRelay) isAgentOnline(agentKey, tenantID, agentType string) bool {
	gbl.ReverseClientsMutex.RLock()
	defer gbl.ReverseClientsMutex.RUnlock()

	if tenantClients, exists := gbl.ReverseClients[tenantID]; exists {
		if agentTypeClients, exists := tenantClients[agentType]; exists {
			if _, exists := agentTypeClients[agentKey]; exists {
				return true
			}
			transformedKey := gbl.Store.GetPrivateKeyUsingPublicKey(agentKey)
			if transformedKey != agentKey {
				if _, exists := agentTypeClients[transformedKey]; exists {
					return true
				}
			}
		}
	}
	return false
}
func (w websocketRelay) broadcastStatusUpdate(msg map[string]interface{}) {
	msgBytes, _ := json.Marshal(msg)
	tenantID, _ := msg["agent_id"].(string)
	agentType, _ := msg["agent_type"].(string)
	if clientsByTenant, ok := gbl.Clients[tenantID]; ok {
		if clientsByType, ok := clientsByTenant["chat_agent"]; ok {
			for _, clients := range clientsByType {
				for _, c := range clients {
					if c != nil {
						data, _ := json.Marshal(msg)
						c.WriteMessage(websocket.TextMessage, data)
					}
				}
			}
		}
	}

	for tenantID, tenantClients := range gbl.ReverseClients {
		if agentTypeClients, exists := tenantClients[agentType]; exists {
			for key, channel := range agentTypeClients {
				DebugPrintf("Broadcasting status update to reverse client - tenant: %s, agentType: %s, key: %s\n", tenantID, agentType, key)
				select {
				case channel <- msgBytes:

				default:
					DebugPrintln("Error: reverse client channel is full or closed")
				}
			}
		}
	}
}
