package relay

import (
	"cpc/gbl"
	"encoding/json"
	"fmt"

	"github.com/gofiber/contrib/websocket"
)

func (w websocketRelay) RemoveClient(tenent_id string, key string, index int, agentType string) {
	gbl.ClientsMutex.Lock()
	defer gbl.ClientsMutex.Unlock()

	if gbl.Clients[tenent_id] != nil && gbl.Clients[tenent_id][agentType] != nil {
		gbl.Clients[tenent_id][agentType][key] = removeIndex(gbl.Clients[tenent_id][agentType][key], index)
		DebugPrintf("existing tenant ids: %v\n", gbl.Clients[tenent_id][agentType])

		if len(gbl.Clients[tenent_id][agentType][key]) == 0 {
			// Pause agent when no clients are connected
			gbl.ReverseClientsMutex.RLock()
			if gbl.ReverseClients[tenent_id] != nil && gbl.ReverseClients[tenent_id][agentType] != nil {
				if reverseConn, exists := gbl.ReverseClients[tenent_id][agentType][key]; exists {
					gbl.ReverseClientsMutex.RUnlock()
					w.agentPause(reverseConn)
				} else {
					gbl.ReverseClientsMutex.RUnlock()
				}
			} else {
				gbl.ReverseClientsMutex.RUnlock()
			}
			delete(gbl.Clients[tenent_id][agentType], key)

			// Clean up empty maps
			if len(gbl.Clients[tenent_id][agentType]) == 0 {
				delete(gbl.Clients[tenent_id], agentType)
				if len(gbl.Clients[tenent_id]) == 0 {
					delete(gbl.Clients, tenent_id)
				}
			}
		}
	}
}

func (w websocketRelay) RemoveReverseClient(tenent_id string, key string, agentType string) {
	gbl.ReverseClientsMutex.Lock()
	defer gbl.ReverseClientsMutex.Unlock()

	DebugPrintf("Removing reverse client - TenantID: %s, AgentType: %s, Key: %s\n", tenent_id, agentType, key)

	if gbl.ReverseClients[tenent_id] != nil && gbl.ReverseClients[tenent_id][agentType] != nil {
		if channel, exists := gbl.ReverseClients[tenent_id][agentType][key]; exists {
			close(channel)
		}

		delete(gbl.ReverseClients[tenent_id][agentType], key)
		DebugPrintf("Reverse client removed for key: %s\n", key)

		if len(gbl.ReverseClients[tenent_id][agentType]) == 0 {
			delete(gbl.ReverseClients[tenent_id], agentType)
			if len(gbl.ReverseClients[tenent_id]) == 0 {
				delete(gbl.ReverseClients, tenent_id)
			}
		}
		if agentType == "chat_agent" {
			w.broadcastAgentOfflineStatus(tenent_id, key, agentType)
		}
	}
}

func (w websocketRelay) broadcastAgentOfflineStatus(tenent_id string, agentKey string, agentType string) {
	DebugPrintf("Broadcasting offline status for agent - TenantID: %s, AgentType: %s, Key: %s\n", tenent_id, agentType, agentKey)

	statusMsg := map[string]interface{}{
		"type":       "status_update",
		"agent_id":   tenent_id,
		"status":     "offline",
		"agent_type": agentType,
	}
	w.broadcastStatusUpdate(statusMsg)
}

func (w websocketRelay) broadcastAgentOnlineStatus(tenent_id string, agentKey string, agentType string) {
	DebugPrintf("Broadcasting online status for agent - TenantID: %s, AgentType: %s, Key: %s\n", tenent_id, agentType, agentKey)

	statusMsg := map[string]interface{}{
		"type":       "status_update",
		"agent_id":   tenent_id,
		"status":     "online",
		"agent_type": agentType,
	}

	w.broadcastStatusUpdate(statusMsg)
}

func (w websocketRelay) sendCurrentAgentStatuses(conn *websocket.Conn, tenant_id string, agentType string) {
	DebugPrintf("Sending current agent statuses to newly connected client - TenantID: %s, AgentType: %s\n", tenant_id, agentType)

	if agentType != "chat_agent" {
		DebugPrintf("Skipping status updates for non-chat agent type: %s\n", agentType)
		return
	}

	gbl.ReverseClientsMutex.RLock()
	if tenantClients, exists := gbl.ReverseClients[tenant_id]; exists {
		if agentTypeClients, exists := tenantClients[agentType]; exists {
			for agentKey := range agentTypeClients {
				DebugPrintf("check online status for agent - TenantID: %s, AgentType: %s, Key: %s\n", tenant_id, agentType, agentKey)
				if w.isAgentOnline(agentKey, tenant_id, agentType) {

					statusMsg := map[string]interface{}{
						"type":       "status_update",
						"agent_id":   tenant_id,
						"status":     "online",
						"agent_type": agentType,
					}

					msgBytes, err := json.Marshal(statusMsg)
					if err == nil {
						conn.WriteMessage(websocket.TextMessage, msgBytes)
						DebugPrintf("Sent online status for agent - TenantID: %s, AgentType: %s, Key: %s\n", tenant_id, agentType, agentKey)
					}
				} else {
					statusMsg := map[string]interface{}{
						"type":       "status_update",
						"agent_id":   tenant_id,
						"status":     "offline",
						"agent_type": agentType,
					}

					msgBytes, err := json.Marshal(statusMsg)
					if err == nil {
						conn.WriteMessage(websocket.TextMessage, msgBytes)
						DebugPrintf("Sent offline status for agent - TenantID: %s, AgentType: %s, Key: %s\n", tenant_id, agentType, agentKey)
					}
				}
			}
		}
	}
	gbl.ReverseClientsMutex.RUnlock()
}
func (w websocketRelay) registerClient(key string, conn *websocket.Conn, tenant_id string, agentType string) {
	fmt.Printf("Registering client - TenantID: %s, AgentType: %s, Key: %s\n", tenant_id, agentType, key)

	gbl.ClientsMutex.Lock()
	if gbl.Clients[tenant_id] == nil {
		gbl.Clients[tenant_id] = make(map[string]map[string][]*websocket.Conn)
	}
	if gbl.Clients[tenant_id][agentType] == nil {
		gbl.Clients[tenant_id][agentType] = make(map[string][]*websocket.Conn)
	}
	if gbl.Clients[tenant_id][agentType][key] == nil {
		gbl.Clients[tenant_id][agentType][key] = []*websocket.Conn{}
		gbl.ReverseClientsMutex.RLock()
		if gbl.ReverseClients[tenant_id] != nil && gbl.ReverseClients[tenant_id][agentType] != nil {
			if reverseConn, exists := gbl.ReverseClients[tenant_id][agentType][key]; exists {
				gbl.ReverseClientsMutex.RUnlock()
				w.agentUnpause(reverseConn)
			} else {
				gbl.ReverseClientsMutex.RUnlock()
			}
		} else {
			gbl.ReverseClientsMutex.RUnlock()
		}
	}
	gbl.Clients[tenant_id][agentType][key] = append(gbl.Clients[tenant_id][agentType][key], conn)
	gbl.ClientsMutex.Unlock()

	if agentType == "chat_agent" {
		w.sendCurrentAgentStatuses(conn, tenant_id, agentType)
	}

	gbl.ClientsMutex.RLock()
	DebugPrintf("Available clients for %s/%s: %v\n", tenant_id, agentType,
		func() []string {
			keys := make([]string, 0)
			for k := range gbl.Clients[tenant_id][agentType] {
				keys = append(keys, k)
			}
			return keys
		}())
	gbl.ClientsMutex.RUnlock()
}

func (w websocketRelay) registerReverseClient(key string, conn chan []byte, tenant_id string, agentType string) {
	DebugPrintf("Registering reverse client - TenantID: %s, AgentType: %s, Key: %s\n", tenant_id, agentType, key)
	existingPublicKey := gbl.Store.GetPublicKeyByDeviceName(tenant_id)
	var publicKeyToUse string
	if existingPublicKey != "no device found" && existingPublicKey != "" {
		publicKeyToUse = existingPublicKey
		DebugPrintf("Reusing existing public key for tenant %s: %s\n", tenant_id, publicKeyToUse)
	} else {
		publicKeyToUse = w.GenerateRandomKey()
		DebugPrintf("Generated new public key for new tenant %s: %s\n", tenant_id, publicKeyToUse)
	}

	gbl.Store.AddAgent(tenant_id, AgentKeyChain{key, publicKeyToUse})

	gbl.ReverseClientsMutex.Lock()
	if gbl.ReverseClients[tenant_id] == nil {
		gbl.ReverseClients[tenant_id] = make(map[string]map[string]chan []byte)
	}
	if gbl.ReverseClients[tenant_id][agentType] == nil {
		gbl.ReverseClients[tenant_id][agentType] = make(map[string]chan []byte)
	}
	DebugPrintf("registering reverse client for agent type: %s\n", agentType)
	gbl.ReverseClients[tenant_id][agentType][key] = conn
	gbl.ReverseClientsMutex.Unlock()

	DebugPrintln("reverse client registered")
	gbl.ReverseClientsMutex.RLock()
	DebugPrintf("Available reverse clients for %s/%s: %v\n", tenant_id, agentType,
		func() []string {
			keys := make([]string, 0)
			for k := range gbl.ReverseClients[tenant_id][agentType] {
				keys = append(keys, k)
			}
			return keys
		}())
	gbl.ReverseClientsMutex.RUnlock()

	if agentType == "chat_agent" {
		w.broadcastAgentOnlineStatus(tenant_id, key, agentType)
	}
}
