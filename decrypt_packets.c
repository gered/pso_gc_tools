/*
 * PSO EP1&2 (Gamecube) Client/Server Packets Decrypter Tool
 *
 * This tool was made for myself as part of an investigative effort to figure out the undocumented "magic" behind
 * what PSO servers have done behind the scenes to prepare .bin/.dat quest files into something that works as an
 * offline/download quest which is playable from a Gamecube memory card.
 *
 * Given two binary files containing server->client and client->server packet data (separately), as long as the
 * packet data was captured from the very beginning of the connection, this will decrypt the packet data and display
 * it as raw packets.
 *
 * Gered King, March 2021
 */

#include <stdio.h>
#include <stdint.h>
#include <malloc.h>

#include <sylverant/encryption.h>

#include "defs.h"
#include "utils.h"

typedef struct _PACKED_ {
	uint8_t pkt_id;
	uint8_t pkt_flags;
	uint16_t pkt_size;
} PACKET_HEADER;

typedef struct _PACKED_ {
	PACKET_HEADER header;
	char message[64];
	uint32_t server_key;
	uint32_t client_key;
	// note: there may be more data. if so, it is likely just more text which can be ignored. check header.pkt_size
} WELCOME_PACKET;

void decrypt_and_display_packets(CRYPT_SETUP *cs, uint8_t *packet_data, size_t size) {
	size_t pos = 0;

	CRYPT_CryptData(cs, packet_data, size, 0);

	while (pos < size) {
		PACKET_HEADER *header = (PACKET_HEADER*)&packet_data[pos];

		printf("id=%x, flags=%x, size=%d\n", header->pkt_id, header->pkt_flags, header->pkt_size);
		CRYPT_PrintData(&packet_data[pos], header->pkt_size);
		printf("\n");

		pos += header->pkt_size;
	}
}

int main(int argc, char *argv[]) {
	int returncode;
	uint8_t *server_data = NULL;
	uint8_t *client_data = NULL;

	if (argc != 3) {
		printf("Usage: decrypt_packets server-packet-data.bin client-packet-data.bin\n");
		return 1;
	}

	const char *server_packet_file = argv[1];
	const char *client_packet_file = argv[2];

	uint32_t server_data_size = 0;
	returncode = read_file(server_packet_file, &server_data, &server_data_size);
	if (returncode) {
		printf("Error code %d (%s) reading server packet data file: %s\n", returncode, get_error_message(returncode), server_packet_file);
		goto error;
	}

	uint32_t client_data_size = 0;
	returncode = read_file(client_packet_file, &client_data, &client_data_size);
	if (returncode) {
		printf("Error code %d (%s) reading client packet data file: %s\n", returncode, get_error_message(returncode), client_packet_file);
		goto error;
	}

	WELCOME_PACKET *welcome = (WELCOME_PACKET*)server_data;
	if (welcome->header.pkt_id != 0x02 && welcome->header.pkt_id != 0x17) {
		printf("Missing or unrecognized 'Welcome' packet:\n\n");
		CRYPT_PrintData(welcome, sizeof(WELCOME_PACKET));
		printf("\nWill not be able to successfully decrypt session. Aborting.\n");
		goto error;
	}

	// read client & server crypt keys from the "Welcome" packet the server sends right away. always unencrypted.
	printf("'Welcome' packet. id=%x, flags=%x, size=%d\n",
	       welcome->header.pkt_id,
	       welcome->header.pkt_flags,
	       welcome->header.pkt_size);
	CRYPT_PrintData(welcome, welcome->header.pkt_size);
	printf("\n");

	printf("server_key = 0x%x\nclient_key = 0x%x\n\n", welcome->server_key, welcome->client_key);

	// set up crypt functionality using those keys, so we can read the rest of the server and client packet data
	// (all of the rest of it will be encrypted)
	CRYPT_SETUP server_cs, client_cs;

	CRYPT_CreateKeys(&server_cs, &welcome->server_key, CRYPT_GAMECUBE);
	CRYPT_CreateKeys(&client_cs, &welcome->client_key, CRYPT_GAMECUBE);

	// display remainder of server packets first
	printf("**** SERVER -> CLIENT PACKETS ****\n\n");
	decrypt_and_display_packets(&server_cs, server_data + welcome->header.pkt_size, server_data_size - welcome->header.pkt_size);

	// now display the client packets
	printf("**** CLIENT -> SERVER PACKETS ****\n\n");
	decrypt_and_display_packets(&client_cs, client_data, client_data_size);

	returncode = 0;
	goto quit;
error:
	returncode = 1;
quit:
	free(server_data);
	free(client_data);
	return returncode;
}
