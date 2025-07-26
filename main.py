import os
import discord
from discord.ext import commands
from dotenv import load_dotenv
from main.database import supabase, store_message
from main.retrieval import generate_response
import asyncio
from concurrent.futures import ThreadPoolExecutor
from main.database import fetch_server_history



executor = ThreadPoolExecutor()

# Load environment variables (the hidden stuff)
load_dotenv()
TOKEN = os.getenv("DISCORD_TOKEN")

# Check for possible error source
if not TOKEN:
    raise ValueError("DISCORD_TOKEN not found")

# Enable intents
intents = discord.Intents.default()
intents.message_content = True

# Dictionary for spam detection
last_messages = {}

bot = commands.Bot(command_prefix='/', intents=intents)

@bot.event
async def on_ready():
    print(f'Logged in as {bot.user}')
    await bot.tree.sync()
    print("Bot is running!")


@bot.event
# Handles incoming messages, stores them in the supabase database with category tracking
async def on_message(message):
    # Ignore messages from the bot itself or DMs to the bot
    if message.author == bot.user or not message.guild:
        return  

    # If the same user sends an identical message within 10 seconds then the message gets ignored
    current_time = message.created_at.timestamp()
    user_id = message.author.id
    if user_id in last_messages:
        last_content, last_time = last_messages[user_id]
        if message.content == last_content and (current_time - last_time) < 10:
            return
        
    # Update the last message record for the user
    last_messages[user_id] = (message.content, current_time)

    # Discord channels don't need to be in a category so make sure it is in one, otherwise no category
    category = message.channel.category.name if message.channel.category else "No Category"

    # Defining the parameters to store the dataset
    # Pass the actual channel object here:
    store_message(
        server_id=message.guild.id,
        author=str(message.author),
        user_id=message.author.id,
        content=message.content,
        category=category,
        channel=message.channel,  
        server=str(message.guild.name),
        timestamp=message.created_at
    )

    # Process commands
    await bot.process_commands(message)

# Fetches recent messages and searches for an answer.
@bot.tree.command(name="asking", description="Ask me anything about this server!")
async def asking(interaction: discord.Interaction, question: str):
    # Defer the response immediately to prevent timeout
    await interaction.response.defer(thinking=True, ephemeral=True)  

    server_id = interaction.guild.id

    try:
        # Run generate_response in an executor to prevent blocking
        loop = asyncio.get_event_loop()
        response = await loop.run_in_executor(executor, generate_response, question, server_id)

        # Send follow-up message once the response is ready
        await interaction.followup.send(response)
    
    except Exception as e:
        await interaction.followup.send(f"An error occurred: {e}", ephemeral=True)

# This function is for admins to clear messages from their servers database, in case of a reset of data, privacy reasons, or if spam is detected
@bot.tree.command(name="clear", description="Clear X amount of recent messages from the database")
async def clear(interaction: discord.Interaction, count: int):
    await interaction.response.defer()
    if not interaction.user.guild_permissions.administrator:
        await interaction.response.send_message("You do not have permission to use this command.", ephemeral=True)
        return

    server_id = str(interaction.guild.id)

    # Fetch latest N chunks for this server
    response = supabase.table("message_chunks")\
        .select("id")\
        .eq("server_id", server_id)\
        .order("timestamp", desc=True)\
        .limit(count)\
        .execute()

    if not response.data:
        await interaction.followup.send("No messages found to clear.")
        return

    ids = [chunk["id"] for chunk in response.data]

    # Delete them
    delete_response = supabase.table("message_chunks")\
        .delete()\
        .in_("id", ids)\
        .execute()

    await interaction.followup.send(f"Deleted {len(ids)} message chunks from the database.", ephemeral=True)

# Fetches a certain number of messages
@bot.tree.command(name="fetch", description="Fetch all previous messages from this server and store them")
async def fetch(interaction: discord.Interaction, count: int):
    await interaction.response.defer()
    if not interaction.user.guild_permissions.administrator:
        await interaction.followup.send("You do not have permission to use this command.", ephemeral=True)
        return

    server_id = interaction.guild.id
    
    await interaction.followup.send(f"🔄 Starting to fetch up to {count} messages per channel from this server. This may take a while...", ephemeral=True)
    
    try:
        await fetch_server_history(bot, server_id, count)
        await interaction.followup.send("✅ Server history fetch completed! All messages have been stored in the database.", ephemeral=True)
    except Exception as e:
        await interaction.followup.send(f"❌ Error fetching server history: {e}", ephemeral=True)

#Fetches all the messages sent
@bot.tree.command(name="fetch-all", description="Fetch all messages from this server and store them **Warning** This might take a very long time")
async def fetch_all(interaction: discord.Interaction):
    await interaction.response.defer()
    if not interaction.user.guild_permissions.administrator:
        await interaction.followup.send("You do not have permission to use this command.", ephemeral=True)
        return
    server_id = interaction.guild.id
    try:
        await fetch_server_history(bot, server_id, None) 
        await interaction.followup.send("✅ All messages have been fetched and stored in the database.", ephemeral=True)
    except Exception as e:
        await interaction.followup.send(f"❌ Error fetching server history: {e}", ephemeral=True)
@bot.tree.command(name="invite", description="Get the bot's invite link!")
async def invite(interaction: discord.Interaction):
    invite_link = "https://discord.com/oauth2/authorize?client_id=1340139928994189322&permissions=8&integration_type=0&scope=bot"
    await interaction.response.send_message(f"Invite me to your server using this link: {invite_link}")



bot.run(TOKEN)
