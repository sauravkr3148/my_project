package jsonstore

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"strings"
)

type JSONStore struct {
	FilePath string
}

func (s *JSONStore) Load() (map[string]interface{}, error) {
	data := make(map[string]interface{})

	fileBytes, err := ioutil.ReadFile(s.FilePath)
	if os.IsNotExist(err) {
		return data, nil
	}
	if err != nil {
		return nil, err
	}
	err = json.Unmarshal(fileBytes, &data)
	return data, err
}

func (s *JSONStore) Save(data map[string]interface{}) error {
	fileBytes, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return err
	}
	return ioutil.WriteFile(s.FilePath, fileBytes, 0644)
}

func (s *JSONStore) AddAgent(agentID string, value interface{}) error {
	data, err := s.Load()
	if err != nil {
		return err
	}
	agents, ok := data["agents"].(map[string]interface{})
	if !ok {
		agents = make(map[string]interface{})
	}

	if existingAgent, exists := agents[agentID]; exists {
		fmt.Printf("Agent '%s' exists. Preserving existing public key.\n", agentID)

		if existingAgentMap, ok := existingAgent.(map[string]interface{}); ok {
			if newAgentMap, ok := value.(map[string]interface{}); ok {
				if existingPublicKey, hasPublicKey := existingAgentMap["publickey"]; hasPublicKey {
					newAgentMap["publickey"] = existingPublicKey
					fmt.Printf("Preserved existing public key: %v\n", existingPublicKey)
				}
			}
		}
	} else {
		fmt.Printf("Agent '%s' does not exist. Adding new agent.\n", agentID)
	}

	agents[agentID] = value

	data["agents"] = agents

	return s.Save(data)
}
func (s *JSONStore) AddUser(userID string, user interface{}) error {
	data, err := s.Load()
	if err != nil {
		return err
	}

	users, ok := data["user"].(map[string]interface{})
	if !ok {
		users = make(map[string]interface{})
	}

	if _, exists := users[userID]; exists {
		fmt.Printf("User '%s' exists. Replacing data.\n", userID)
	} else {
		fmt.Printf("User '%s' does not exist. Adding new user.\n", userID)
	}

	users[userID] = user
	data["user"] = users

	return s.Save(data)
}

func (s *JSONStore) AddAgentPermission(userId string, permission interface{}) error {
	data, err := s.Load()
	if err != nil {
		return err
	}
	token := s.GetTokenByUser(userId)
	perms, ok := data["agent_permission"].(map[string]interface{})
	if !ok {
		perms = make(map[string]interface{})
	}

	if _, exists := perms[userId]; exists {
		fmt.Printf("Permission for agent '%s' exists. Replacing it.\n", userId)
	} else {
		fmt.Printf("Permission for agent '%s' does not exist. Adding new permission.\n", userId)
	}

	perms[token] = permission
	data["agent_permission"] = perms

	return s.Save(data)
}

func (s *JSONStore) Delete(key string) error {
	data, err := s.Load()
	if err != nil {
		return err
	}
	delete(data, key)
	return s.Save(data)
}

func (s *JSONStore) AgentKeys() ([]string, error) {
	data, err := s.Load()
	if err != nil {
		return nil, err
	}

	agentsRaw, ok := data["agents"]
	if !ok {
		return []string{}, nil
	}

	agents, ok := agentsRaw.(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("'agents' is not a valid object")
	}

	keys := make([]string, 0, len(agents))
	for key := range agents {
		keys = append(keys, key)
	}

	return keys, nil
}

func (s *JSONStore) UserKeysAndFullNames() (map[string]string, error) {
	data, err := s.Load()
	if err != nil {
		return nil, err
	}

	usersRaw, ok := data["user"]
	if !ok {
		return map[string]string{}, nil
	}

	users, ok := usersRaw.(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("'users' is not a valid object")
	}

	userMap := make(map[string]string)
	for key, val := range users {
		userData, ok := val.(map[string]interface{})
		if !ok {
			continue
		}

		fullName, ok := userData["fullName"].(string)
		if !ok {
			fullName = ""
		}

		userMap[fullName] = key
	}

	return userMap, nil
}

func (s *JSONStore) GetUser() []string {
	maps, err := s.UserKeysAndFullNames()
	if err != nil {
		fmt.Println(err)
	}
	keys := make([]string, 0, len(maps))
	for k := range maps {
		keys = append(keys, k)
	}
	return keys
}

func (s *JSONStore) GetTokenByUser(name string) string {
	maps, err := s.UserKeysAndFullNames()
	if err != nil {
		fmt.Println(err)
	}
	return maps[name]
}

func (s *JSONStore) AgentPermission(session string) (map[string]interface{}, error) {
	data, err := s.Load()
	if err != nil {
		return nil, err
	}

	permissionRaw, ok := data["agent_permission"]
	if !ok {
		fmt.Println("failed to get raw permission")
	}

	permission, ok := permissionRaw.(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("'users' is not a valid object")
	}
	return permission, nil

}

func (s *JSONStore) GetDevicesByAllowedUser(authToken string) ([]string, error) {
	permission, err := s.AgentPermission(authToken)
	if err != nil {
		return nil, fmt.Errorf("failed to get agent permission: %w", err)
	}

	var devices []string
	for _, val := range permission {
		userData, ok := val.(map[string]interface{})
		if !ok {
			continue
		}
		deviceStr, ok := userData["Devices"].(string)
		if !ok {
			fmt.Println("Devices field is not a string")
			continue
		}
		deviceList := strings.Split(deviceStr, ",")
		for _, device := range deviceList {
			device = strings.TrimSpace(device)
			if device != "" {
				devices = append(devices, device)
			}
		}
	}

	fmt.Println("list of devices:", devices)
	return devices, nil
}

func (s *JSONStore) DeviceList() (map[string]interface{}, error) {
	data, err := s.Load()
	if err != nil {
		return nil, err
	}

	deviceRaw, ok := data["agents"]
	if !ok {
		fmt.Println("failed to get raw permission")
	}

	devices, ok := deviceRaw.(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("'users' is not a valid object")
	}
	return devices, nil

}

func (s *JSONStore) GetPublicKeyByDeviceName(name string) string {
	devices, err := s.DeviceList()
	if err != nil {
		fmt.Println(err)
	}
	for key, val := range devices {

		if key == name {
			userData, ok := val.(map[string]interface{})
			if !ok {
				fmt.Println("was skipped")
				continue
			}
			pubKey, ok := userData["publickey"].(string)
			if !ok {
				fmt.Println("Devices field is not a string")
				continue
			}
			return pubKey

		}

	}
	return "no device found"

}

func (s *JSONStore) GetPrivateKeyUsingPublicKey(name string) string {
	devices, err := s.DeviceList()
	if err != nil {
		fmt.Println(err)
	}
	for _, val := range devices {
		userData, ok := val.(map[string]interface{})
		if !ok {
			continue
		}
		pubKey, ok := userData["publickey"].(string)
		if !ok {
			fmt.Println("Devices field is not a string")
			continue
		}
		if pubKey == name {
			privKey, ok := userData["key"].(string)
			if !ok {
				fmt.Println("Devices field is not a string")
				continue
			}
			return privKey
		}

	}
	return "no device found"

}
